//! The `replay` tool must record what the game holds, and say what it could not record.
//!
//! `RecordingBuffer::fetch_entities` used to return an empty map with a `TODO`, so a recording held
//! no state at all. The tool still reported "Recording started" and a frame count, so an agent
//! looked at a recording of nothing and read it as a recording of the game.
//!
//! Entity states are observable over BRP and are now recorded. Events are not: BRP has no method
//! that reports the events a game has seen. A recording has to say that, or an empty event list
//! reads as "nothing happened" when it means "we cannot see events".

use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::post, Json, Router};
use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::recording_system::RecordingBuffer;
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
        "world.query" => Ok(json!([
            {"entity": 1, "components": {"Transform": {"x": 1.0}}},
            {"entity": 2, "components": {"Health": 100}}
        ])),
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

/// A recorded frame must hold the entities the game has. The fake game has two.
#[tokio::test]
async fn a_recorded_frame_holds_the_games_entities() {
    let game: SharedGame = Arc::new(Mutex::new(FakeGame::default()));
    let port = start_fake_game(game.clone()).await;
    let client = client_for(port);

    let mut buffer = RecordingBuffer::new(bevy_debugger_mcp::recording_system::RecordingConfig::default());
    buffer.start_recording();

    let mut client_guard = client.write().await;
    buffer.record_frame(&mut client_guard).await.expect("the frame is recorded");

    let frames = buffer.get_frames();
    assert_eq!(frames.len(), 1, "one frame was recorded");
    let frame = &frames[0];
    assert_eq!(
        frame.entities.len(),
        2,
        "the frame must hold the game's entities, not an empty map; frame: {frame:?}"
    );
    assert!(frame.entities.contains_key(&1), "entity 1 must be in the frame");
    assert!(frame.entities.contains_key(&2), "entity 2 must be in the frame");
    assert_eq!(
        frame.entities[&1].components.get("Transform"),
        Some(&json!({"x": 1.0})),
        "the frame must hold the entity's components"
    );

    let game = game.lock().expect("fake game lock");
    assert!(
        game.calls.contains(&"world.query".to_string()),
        "the frame must come from asking the game; calls were {:?}",
        game.calls
    );
}

/// A frame must say that events were not captured, so an empty event list is never read as
/// "nothing happened".
#[tokio::test]
async fn a_frame_says_that_events_were_not_captured() {
    let game: SharedGame = Arc::new(Mutex::new(FakeGame::default()));
    let port = start_fake_game(game.clone()).await;
    let client = client_for(port);

    let mut buffer = RecordingBuffer::new(bevy_debugger_mcp::recording_system::RecordingConfig::default());
    buffer.start_recording();

    let mut client_guard = client.write().await;
    buffer.record_frame(&mut client_guard).await.expect("the frame is recorded");

    let frames = buffer.get_frames();
    let frame = &frames[0];
    assert!(
        frame.events.is_empty(),
        "BRP reports no events, so the list is empty"
    );
    let reason = frame
        .events_not_captured
        .as_deref()
        .expect("the frame must say why the event list is empty");
    assert!(
        reason.contains("events") && reason.len() > 20,
        "the reason must name what was not captured; got {reason}"
    );
}

/// The tool must say what it captures and what it does not, when the recording starts.
#[tokio::test]
async fn the_record_report_says_what_is_not_captured() {
    let game: SharedGame = Arc::new(Mutex::new(FakeGame::default()));
    let port = start_fake_game(game.clone()).await;
    let client = client_for(port);

    let tools = bevy_debugger_mcp::mcp_tools::BevyDebuggerTools::new(client);
    let status = tools
        .status(Parameters(bevy_debugger_mcp::mcp_tools::NoArgs {}))
        .await
        .expect("status returns a result");
    assert!(!text_of(&status).contains("\"error\""), "the fake game should answer status");

    let result = tools
        .replay(Parameters(serde_json::from_value(json!({"action": "record"})).expect("the request deserialises")))
        .await
        .expect("replay returns a result");

    let text = text_of(&result);
    let value: Value = serde_json::from_str(&text).expect("the reply is JSON");
    let not_captured = value
        .get("not_captured")
        .and_then(|v| v.get("events"))
        .and_then(Value::as_str)
        .expect("the report must say that events are not captured");
    assert!(
        not_captured.contains("not observable") || not_captured.contains("not captured"),
        "the report must be clear that an empty event list is not \"nothing happened\"; got {not_captured}"
    );
}
