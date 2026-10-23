/*
 * bevy-fairy - the guide channel
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 */

//! **Working *with* the person at the keyboard, rather than only on their app.**
//!
//! Every other tool in this server points at the game: query it, poke it, measure it. This one
//! points at the human. An agent posts a short script; the app renders one step at a time on the
//! window the person is already looking at; the plugin watches a named condition and records what
//! actually happened.
//!
//! It exists because the loop without it is bad in a specific way. The person hits something, writes
//! prose about it, and the agent guesses at the key sequence — five reports in one afternoon, three
//! not reproduced first time, and four occasions of the panel text being pasted back by hand because
//! an offscreen capture cannot see a UI tree.
//!
//! # The half an agent gets wrong
//!
//! **A step with no checkpoint never advances on its own, and the app cannot tell the person what to
//! do about it.** That is not a defect: "does this look right?" is not a machine question, and the
//! whole reason a person is in the loop is to answer it. But it means the agent has an obligation the
//! other tools do not carry — when a step is waiting on a person, *say so in the conversation*, in
//! words, and wait. The app says `-> yours to judge. Nothing here advances it: tell the agent.` and
//! that promise is the agent's to keep.
//!
//! An agent that posts a script and goes quiet strands somebody in front of a card that will never
//! move. That happened during this tool's own development, to the person it was built for.

use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::server::brp::{refusal_text, BrpPayload, BrpRequest};
use crate::server::brp_client::BrpClient;
use crate::server::error::Error;

/// The BRP method the companion plugin registers.
const GUIDE_METHOD: &str = "bevy_fairy/guide";

/// Forward a guide request to the running app and hand back its answer.
///
/// Thin on purpose: the plugin owns the script, the transcript and the overlay, so a second copy of
/// any of that here would be a second source of truth for the state a person is looking at.
///
/// The error is boxed: `Error` is large enough that clippy's `result_large_err` flags returning it
/// by value, and this is the one place in this file that returns it.
pub async fn handle(
    arguments: Value,
    brp_client: Arc<RwLock<BrpClient>>,
) -> std::result::Result<Value, Box<Error>> {
    debug!("guide request: {arguments}");

    let request = BrpRequest {
        method: GUIDE_METHOD.to_string(),
        id: Some(Value::from(1)),
        params: Some(arguments),
    };

    let response = {
        let mut client = brp_client.write().await;
        client.send_request(&request).await?
    };

    match response.payload {
        BrpPayload::Result(value) => Ok(value),
        // `refusal_text` names the feature that registers the method, because "method not found"
        // has exactly one cause here and an agent that reads it bare debugs its own JSON instead.
        BrpPayload::Error(err) => Err(Box::new(Error::Validation(format!(
            "the app refused `{GUIDE_METHOD}`: {}",
            refusal_text(GUIDE_METHOD, &err)
        )))),
    }
}
