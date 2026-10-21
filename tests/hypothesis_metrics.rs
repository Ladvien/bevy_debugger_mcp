//! The `hypothesis` tool must not report numbers it did not measure.
//!
//! `PerformanceMetric::measure` used to return a constant for every variant: 16.67 ms of frame
//! time, 100 entities, 500 components, 1024 MB of memory, 50% CPU. An agent reading "entity count:
//! 100" could not tell it from a measurement of a game that has two entities, and the constant
//! made any assertion built on it meaningless.
//!
//! Two metrics can be observed over BRP: the entity count and the component count. The other
//! three have no way to be observed from here, so they must be refused with a reason.

use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::post, Json, Router};
use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::hypothesis_system::PerformanceMetric;
use rmcp::handler::server::tool::Parameters;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

#[derive(Default)]
struct FakeGame {
    calls: Vec<String>,
}

type SharedGame = Arc<Mutex<FakeGame>>;

async fn answer(State(game): State<SharedGame>, body: String) -> Json<Value> {
    let request: Value = serde_json::from_str(&body).expect("the client sends JSON");
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or_default().to_string();

    {
        let mut game = game.lock().expect("fake game lock");
        game.calls.push(method.clone());
    }

    let result = match method.as_str() {
        "rpc.discover" => Ok(json!({})),
        // Two entities, three components between them.
        "world.query" => Ok(json!([
            {"entity": 1, "components": {"Transform": {"x": 1.0}, "Health": 100}},
            {"entity": 2, "components": {"Transform": {"x": 2.0}}}
        ])),
        "world.get_components" => Ok(json!({"components": {}})),
        "world.spawn_entity" => Ok(json!({"entity": 3})),
        "world.despawn_entity" => Ok(json!({"entity": 3})),
        "world.insert_components" => Ok(json!({"entity": 3, "success": true})),
        other => Err((-32601, format!("method not found: {other}"))),
    };

    Json(match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
        Err((code, message)) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
        }
    })
}

async fn start_fake_game(game: SharedGame) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind the fake game port");
    let port = listener.local_addr().expect("fake game address").port();
    let app = Router::new().route("/", post(answer)).with_state(game);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    port
}

fn client_for(port: u16) -> Arc<RwLock<BrpClient>> {
    let config = Config {
        bevy_brp_host: "127.0.0.1".to_string(),
        bevy_brp_port: port,
        ..Config::default()
    };
    Arc::new(RwLock::new(BrpClient::new(&config)))
}

fn text_of(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|item| item.as_text().map(|text| text.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The entity count must come from the game. The fake game has two entities; the old code
/// answered 100 without asking.
#[tokio::test]
async fn entity_count_is_measured_from_the_game() {
    let game: SharedGame = Arc::new(Mutex::new(FakeGame::default()));
    let port = start_fake_game(game.clone()).await;
    let client = client_for(port);

    let mut client_guard = client.write().await;
    let measured = PerformanceMetric::EntityCount.measure(&mut client_guard).await;

    assert!(
        measured.is_ok(),
        "the entity count is observable over BRP, so it must be measured; got {measured:?}"
    );
    assert_eq!(
        measured.expect("the count is measured"),
        2.0,
        "the count must be the game's, not a constant"
    );

    let game = game.lock().expect("fake game lock");
    assert!(
        game.calls.contains(&"world.query".to_string()),
        "the count must come from asking the game; calls were {:?}",
        game.calls
    );
}

/// The component count must come from the game too. The fake game has three components across
/// its two entities; the old code answered 500.
#[tokio::test]
async fn component_count_is_measured_from_the_game() {
    let game: SharedGame = Arc::new(Mutex::new(FakeGame::default()));
    let port = start_fake_game(game.clone()).await;
    let client = client_for(port);

    let mut client_guard = client.write().await;
    let measured = PerformanceMetric::ComponentCount.measure(&mut client_guard).await;

    assert!(
        measured.is_ok(),
        "the component count is observable over BRP, so it must be measured; got {measured:?}"
    );
    assert_eq!(
        measured.expect("the count is measured"),
        3.0,
        "the count must be the game's, not a constant"
    );
}

/// The three metrics the protocol cannot report must be refused with a reason, not answered with
/// a number. An agent that asks for frame time needs to know it cannot be had, not a plausible
/// 16.67 ms that looks like a measurement.
#[tokio::test]
async fn metrics_the_protocol_cannot_report_are_refused_with_a_reason() {
    let game: SharedGame = Arc::new(Mutex::new(FakeGame::default()));
    let port = start_fake_game(game.clone()).await;
    let client = client_for(port);

    for metric in [
        PerformanceMetric::FrameTime,
        PerformanceMetric::MemoryUsage,
        PerformanceMetric::CpuUsage,
    ] {
        let mut client_guard = client.write().await;
        let measured = metric.measure(&mut client_guard).await;
        assert!(
            measured.is_err(),
            "{metric:?} cannot be observed over BRP, so it must be refused, not answered"
        );
        let reason = measured.expect_err("the metric is refused").to_string();
        assert!(
            reason.contains("cannot be measured"),
            "the refusal must say what is wrong; got {reason}"
        );
        assert!(
            reason.len() > 40,
            "the refusal must say why, not just that it failed; got {reason}"
        );
    }
}

/// The refusal has to reach the agent as text, not be swallowed by the tool wrapper.
#[tokio::test]
async fn a_refused_metric_reaches_the_agent() {
    let game: SharedGame = Arc::new(Mutex::new(FakeGame::default()));
    let port = start_fake_game(game.clone()).await;
    let client = client_for(port);

    // The `hypothesis` tool with a `PerformanceWithin` assertion on frame time, and one action to
    // run. The action matters: the runner only evaluates the condition after the actions succeed,
    // so without one the condition is never reached and the refusal never happens.
    let tools = bevy_debugger_mcp::mcp_tools::BevyDebuggerTools::new(client);
    let status = tools
        .status(Parameters(bevy_debugger_mcp::mcp_tools::NoArgs {}))
        .await
        .expect("status returns a result");
    assert!(!text_of(&status).contains("\"error\""), "the fake game should answer status");

    let outcome = tools
        .hypothesis(Parameters(serde_json::from_value(json!({
            "action": "test",
            "description": "frame time stays under 16 ms",
            "success_condition": {
                "type": "performance_within",
                "metric": "frame_time",
                "max_value": 16.0
            },
            "actions": [
                {"type": "spawn", "components": [{"type_id": "Transform", "value": {"x": 1.0}}]}
            ],
            "iterations": 1,
            "timeout": 5,
        })).expect("the request deserialises")))
        .await;

    // The refusal is an error, not a result: the tool could not do what was asked, and saying so
    // in the error is the honest shape. The message has to name the reason.
    match outcome {
        Ok(result) => {
            let text = text_of(&result);
            assert!(
                text.contains("cannot be measured"),
                "the agent must be told that frame time cannot be measured; got {text}"
            );
            assert!(
                !text.contains("16.67"),
                "the fabricated 60fps number must not appear anywhere in the reply; got {text}"
            );
        }
        Err(error) => {
            let message = error.to_string();
            assert!(
                message.contains("cannot be measured"),
                "the agent must be told that frame time cannot be measured; got {message}"
            );
            assert!(
                !message.contains("16.67"),
                "the fabricated 60fps number must not appear anywhere in the reply; got {message}"
            );
        }
    }
}
