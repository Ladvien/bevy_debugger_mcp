//! Ground truth: the server's tools, checked against a game whose state is known.
//!
//! The game is `examples/fixture_game.rs`, run in-process on a free port. Every expected value
//! comes from that file's constants -- [`CRATE_WEIGHTS`], [`SCORE_POINTS`] -- and never from the
//! tool under test. The tools are called the way an agent's call arrives: as JSON, deserialised
//! into the tool's own request struct, over a real BRP connection to a real Bevy app.

#[path = "../examples/fixture_game.rs"]
mod fixture_game;

use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy_fairy::server::brp_client::BrpClient;
use bevy_fairy::server::config::Config;
use bevy_fairy::server::mcp_tools::{BevyDebuggerTools, BrpCallRequest, NoArgs};
use fixture_game::{CRATE_WEIGHTS, SCORE_POINTS};
use rmcp::handler::server::tool::Parameters;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};
use tokio::sync::RwLock;

/// A port nobody is listening on, so parallel test binaries do not collide.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .expect("read the ephemeral port")
        .port()
}

/// Start the fixture game on its own thread. It runs until the test process exits.
fn start_fixture() -> u16 {
    let port = free_port();
    std::thread::spawn(move || {
        fixture_game::build_app(port, fixture_game::DEFAULT_FPS).run();
    });
    port
}

fn tools_for(port: u16) -> BevyDebuggerTools {
    let config = Config {
        bevy_brp_host: "127.0.0.1".to_string(),
        bevy_brp_port: port,
        ..Config::default()
    };
    BevyDebuggerTools::new(Arc::new(RwLock::new(BrpClient::new(&config))))
}

fn text_of(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|item| item.as_text().map(|text| text.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Call the `brp` tool the way an agent does, and parse the JSON it returns.
async fn brp(tools: &BevyDebuggerTools, request: Value) -> Value {
    let params: BrpCallRequest = serde_json::from_value(request).expect("the request deserialises");
    let result = tools.brp(Parameters(params)).await.expect("the brp tool returns a result");
    let text = text_of(&result);
    assert_ne!(
        result.is_error,
        Some(true),
        "the brp tool reported an error against the fixture: {text}"
    );
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("brp returned non-JSON ({e}): {text}"))
}

/// Wait until the `status` tool says the fixture is ready. This is itself a check: a game that
/// is up and has finished `Startup` must be reported ready, with a frame count that moved.
async fn wait_until_ready(tools: &BevyDebuggerTools) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let result = tools.status(Parameters(NoArgs {})).await.expect("status returns a result");
        let text = text_of(&result);
        if result.is_error != Some(true) {
            if let Ok(status) = serde_json::from_str::<Value>(&text) {
                if status.get("ready") == Some(&json!(true))
                    && status.get("frame").and_then(Value::as_u64).unwrap_or(0) > 0
                {
                    return status;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "the fixture never reported ready within 60 s; last status: {text}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The `brp` tool's `world.query` returns exactly the crates the fixture spawned, with exactly
/// their weights -- including the deliberately odd one.
#[tokio::test(flavor = "multi_thread")]
async fn brp_query_returns_exactly_the_crates_the_fixture_spawned() {
    let port = start_fixture();
    let tools = tools_for(port);
    wait_until_ready(&tools).await;

    let rows = brp(
        &tools,
        json!({
            "method": "world.query",
            "params": {"data": {"components": ["fixture_game::Crate"]}}
        }),
    )
    .await;
    let rows = rows.as_array().expect("world.query returns an array of rows");

    let mut weights: Vec<f64> = rows
        .iter()
        .map(|row| {
            row["components"]["fixture_game::Crate"]["weight"]
                .as_f64()
                .unwrap_or_else(|| panic!("a crate row has no weight: {row}"))
        })
        .collect();
    weights.sort_by(f64::total_cmp);

    let mut expected: Vec<f64> = CRATE_WEIGHTS.iter().map(|w| f64::from(*w)).collect();
    expected.sort_by(f64::total_cmp);

    assert_eq!(
        weights, expected,
        "the crates the tool reports must be the crates the fixture spawned"
    );
    assert!(
        weights.iter().any(|w| *w < 0.0),
        "the odd value (a negative weight) must come back as it is, not cleaned up"
    );
}

/// The `brp` tool's `world.get_resources` returns the resource's real value.
#[tokio::test(flavor = "multi_thread")]
async fn brp_get_resources_returns_the_fixtures_score() {
    let port = start_fixture();
    let tools = tools_for(port);
    wait_until_ready(&tools).await;

    let reply = brp(
        &tools,
        json!({
            "method": "world.get_resources",
            "params": {"resource": "fixture_game::Score"}
        }),
    )
    .await;

    assert_eq!(
        reply["value"]["points"].as_u64(),
        Some(u64::from(SCORE_POINTS)),
        "the score the tool reports must be the fixture's: {reply}"
    );
}

/// Read every row of one fixture component through the `brp` tool.
async fn rows_of(tools: &BevyDebuggerTools, component: &str) -> Vec<Value> {
    let rows = brp(
        tools,
        json!({"method": "world.query", "params": {"data": {"components": [component]}}}),
    )
    .await;
    rows.as_array().expect("world.query returns an array of rows").clone()
}

/// The planted bug is real, and the tool shows it: once the spinner has run more than
/// [`SPINNER_STOPS_AFTER_FRAMES`] frames, its angle is frozen at exactly that many steps while
/// its frame counter keeps climbing. An eval task's answer depends on this, so the fixture's
/// claim about its own bug is checked, not assumed.
#[tokio::test(flavor = "multi_thread")]
async fn the_planted_spinner_bug_is_visible_through_the_brp_tool() {
    use fixture_game::{SPINNER_STEP, SPINNER_STOPS_AFTER_FRAMES};

    let port = start_fixture();
    let tools = tools_for(port);
    wait_until_ready(&tools).await;

    let deadline = Instant::now() + Duration::from_secs(60);
    let spinner = loop {
        let rows = rows_of(&tools, "fixture_game::Spinner").await;
        assert_eq!(rows.len(), 1, "the fixture spawns one spinner: {rows:?}");
        let spinner = rows[0]["components"]["fixture_game::Spinner"].clone();
        let frames = spinner["frames"].as_u64().expect("the spinner reports its frames");
        if frames > u64::from(SPINNER_STOPS_AFTER_FRAMES) + 10 {
            break spinner;
        }
        assert!(Instant::now() < deadline, "the spinner never passed its stop frame");
        tokio::time::sleep(Duration::from_millis(200)).await;
    };

    let angle = spinner["angle"].as_f64().expect("the spinner reports its angle");
    let frozen = f64::from(SPINNER_STEP) * f64::from(SPINNER_STOPS_AFTER_FRAMES);
    assert!(
        (angle - frozen).abs() < 1e-3,
        "the angle must be frozen at {frozen} after the stop frame; the tool reports {angle} ({spinner})"
    );
}

/// Metamorphic: the mover's x only grows. Two reads a moment apart must show it further along
/// +x, so a tool that returns a cached or stale snapshot fails here.
#[tokio::test(flavor = "multi_thread")]
async fn the_mover_is_further_along_on_a_later_read() {
    let port = start_fixture();
    let tools = tools_for(port);
    wait_until_ready(&tools).await;

    let x_now = |rows: Vec<Value>| -> f64 {
        assert_eq!(rows.len(), 1, "the fixture spawns one mover: {rows:?}");
        rows[0]["components"]["bevy_transform::components::transform::Transform"]["translation"][0]
            .as_f64()
            .unwrap_or_else(|| panic!("the mover's transform has no x: {}", rows[0]))
    };
    let request = json!({
        "method": "world.query",
        "params": {
            "data": {"components": ["bevy_transform::components::transform::Transform"]},
            "filter": {"with": ["fixture_game::Mover"]}
        }
    });
    let first = x_now(brp(&tools, request.clone()).await.as_array().cloned().unwrap_or_default());
    tokio::time::sleep(Duration::from_millis(500)).await;
    let second = x_now(brp(&tools, request).await.as_array().cloned().unwrap_or_default());

    assert!(
        second > first,
        "the mover travels +x at {} units/s, so a later read must be further along: {first} then {second}",
        fixture_game::MOVER_SPEED
    );
}
