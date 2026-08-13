/*
 * Bevy Debugger MCP Server - the guide channel
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

use crate::brp::{BrpPayload, BrpRequest};
use crate::brp_client::BrpClient;
use crate::error::{Error, Result};

/// The BRP method the companion plugin registers.
const GUIDE_METHOD: &str = "bevy_debugger/guide";

/// Forward a guide request to the running app and hand back its answer.
///
/// Thin on purpose: the plugin owns the script, the transcript and the overlay, so a second copy of
/// any of that here would be a second source of truth for the state a person is looking at.
pub async fn handle(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
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
        // **Name the likely cause, because there is exactly one that matters.** The method is
        // registered by `DebuggerPlugin`, which the game and the editor only add behind their
        // `debugger` feature. An agent that reads "method not found" and starts debugging its own
        // JSON is looking in the wrong place.
        BrpPayload::Error(err) => Err(Error::Validation(format!(
            "the app refused `{GUIDE_METHOD}`: {} — if this says the method is unknown, the app is \
             running without DebuggerPlugin (the game needs `--features debugger`; the editor has \
             it on by default)",
            err.message
        ))),
    }
}
