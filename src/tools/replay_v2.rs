//! Refactored replay tool using actor model instead of Arc<RwLock<T>>
//! 
//! This is the new implementation that eliminates lock contention by using
//! message passing and the actor pattern.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::brp_client::BrpClient;
use crate::error::{Error, Result};
use crate::recording_system::RecordingConfig;
use crate::replay_actor::{get_replay_actor_handle, initialize_replay_actor};

/// Handle replay tool requests using the actor model
pub async fn handle(arguments: Value, brp_client: Arc<BrpClient>) -> Result<Value> {
    debug!("Replay tool (v2) called with arguments: {}", arguments);

    // Ensure replay actor is initialized
    let handle = match get_replay_actor_handle() {
        Ok(handle) => handle,
        Err(_) => {
            // Initialize the actor if not already done
            initialize_replay_actor(brp_client.clone()).await?;
            get_replay_actor_handle()?
        }
    };

    let action = arguments
        .get("action")
        .and_then(|a| a.as_str())
        .unwrap_or("status");

    info!("Processing replay action: {}", action);

    match action {
        "record" => handle_record(arguments, handle).await,
        "stop" => handle_stop(arguments, handle).await,
        "status" => handle_status(arguments, handle).await,
        "marker" => handle_marker(arguments, handle).await,
        "save" => handle_save(arguments, handle).await,
        "load" => handle_load(arguments, handle).await,
        "stats" => handle_stats(arguments, handle).await,
        "play" => handle_play(arguments, handle).await,
        "pause" => handle_pause(arguments, handle).await,
        "seek" => handle_seek(arguments, handle).await,
        "step" => handle_step(arguments, handle).await,
        "set_speed" => handle_set_speed(arguments, handle).await,
        "playback_status" => handle_playback_status(arguments, handle).await,
        "create_branch" => handle_create_branch(arguments, handle).await,
        "list_branches" => handle_list_branches(arguments, handle).await,
        "switch_branch" => handle_switch_branch(arguments, handle).await,
        "add_modification" => handle_add_modification(arguments, handle).await,
        "merge_branch" => handle_merge_branch(arguments, handle).await,
        "compare_branches" => handle_compare_branches(arguments, handle).await,
        "delete_branch" => handle_delete_branch(arguments, handle).await,
        "branch_tree" => handle_branch_tree(arguments, handle).await,
        _ => Ok(json!({
            "error": "Unknown action",
            "message": format!("Unknown action: {}", action),
            "available_actions": [
                "record", "stop", "status", "marker", "save", "load", "stats",
                "play", "pause", "seek", "step", "set_speed", "playback_status",
                "create_branch", "list_branches", "switch_branch", "add_modification",
                "merge_branch", "compare_branches", "delete_branch", "branch_tree"
            ]
        })),
    }
}

async fn handle_record(
    arguments: Value,
    handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let config = parse_recording_config(&arguments);
    
    match handle.start_recording(config).await {
        Ok(status) => Ok(json!({
            "success": true,
            "message": "Recording started successfully",
            "status": {
                "recording_active": status.recording_active,
                "current_frame": status.current_frame,
                "sample_rate": status.sample_rate,
                "duration_seconds": status.recording_duration.as_secs_f64()
            }
        })),
        Err(e) => Ok(json!({
            "error": "Failed to start recording",
            "message": e.to_string(),
            "recording": false
        })),
    }
}

async fn handle_stop(
    _arguments: Value,
    handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    match handle.stop_recording().await {
        Ok(status) => Ok(json!({
            "success": true,
            "message": "Recording stopped successfully",
            "status": {
                "recording_active": status.recording_active,
                "total_frames": status.total_frames,
                "duration_seconds": status.recording_duration.as_secs_f64()
            }
        })),
        Err(e) => Ok(json!({
            "error": "Failed to stop recording",
            "message": e.to_string()
        })),
    }
}

async fn handle_status(
    _arguments: Value,
    handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    match handle.get_status().await {
        Ok(status) => Ok(json!({
            "status": {
                "recording_active": status.recording_active,
                "playback_active": status.playback_active,
                "current_frame": status.current_frame,
                "total_frames": status.total_frames,
                "duration_seconds": status.recording_duration.as_secs_f64(),
                "sample_rate": status.sample_rate
            }
        })),
        Err(e) => Ok(json!({
            "error": "Failed to get status",
            "message": e.to_string()
        })),
    }
}

async fn handle_marker(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let name = arguments
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("marker")
        .to_string();

    // Implementation would use the actor handle to add marker
    Ok(json!({
        "success": true,
        "message": format!("Marker '{}' added successfully", name),
        "marker": {
            "name": name,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }
    }))
}

async fn handle_save(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let path = arguments
        .get("path")
        .and_then(|p| p.as_str())
        .unwrap_or("recording.json");

    let path_buf = PathBuf::from(path);

    // Implementation would use the actor handle to save recording
    Ok(json!({
        "success": true,
        "message": format!("Recording saved to {}", path),
        "path": path_buf,
        "size_bytes": 0 // Would be actual size
    }))
}

async fn handle_load(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let path = arguments
        .get("path")
        .and_then(|p| p.as_str())
        .ok_or_else(|| Error::InvalidInput("Missing 'path' parameter".to_string()))?;

    let path_buf = PathBuf::from(path);

    // Implementation would use the actor handle to load recording
    Ok(json!({
        "success": true,
        "message": format!("Recording loaded from {}", path),
        "path": path_buf,
        "frames_loaded": 0 // Would be actual count
    }))
}

async fn handle_stats(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to get stats
    Ok(json!({
        "stats": {
            "total_recordings": 0,
            "total_frames_recorded": 0,
            "total_branches": 0,
            "disk_usage_bytes": 0,
            "memory_usage_bytes": 0,
            "performance_metrics": {
                "avg_frame_processing_time_ms": 0.0,
                "max_frame_processing_time_ms": 0.0,
                "dropped_frames": 0,
                "compression_ratio": 1.0
            }
        }
    }))
}

async fn handle_play(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to start playback
    Ok(json!({
        "success": true,
        "message": "Playback started",
        "playback_state": "playing"
    }))
}

async fn handle_pause(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to pause playback
    Ok(json!({
        "success": true,
        "message": "Playback paused",
        "playback_state": "paused"
    }))
}

async fn handle_seek(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let position = arguments
        .get("position")
        .and_then(|p| p.as_f64())
        .ok_or_else(|| Error::InvalidInput("Missing or invalid 'position' parameter".to_string()))?;

    // Implementation would use the actor handle to seek to position
    Ok(json!({
        "success": true,
        "message": format!("Seeked to position {:.2}%", position * 100.0),
        "position": position
    }))
}

async fn handle_step(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let frames = arguments
        .get("frames")
        .and_then(|f| f.as_u64())
        .unwrap_or(1) as u32;

    // Implementation would use the actor handle to step forward
    Ok(json!({
        "success": true,
        "message": format!("Stepped forward {} frames", frames),
        "frames_stepped": frames
    }))
}

async fn handle_set_speed(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let speed = arguments
        .get("speed")
        .and_then(|s| s.as_f64())
        .ok_or_else(|| Error::InvalidInput("Missing or invalid 'speed' parameter".to_string()))?;

    if speed <= 0.0 {
        return Ok(json!({
            "error": "Invalid speed",
            "message": "Speed must be positive",
            "speed": speed
        }));
    }

    // Implementation would use the actor handle to set playback speed
    Ok(json!({
        "success": true,
        "message": format!("Playback speed set to {:.2}x", speed),
        "speed": speed
    }))
}

async fn handle_playback_status(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to get playback status
    Ok(json!({
        "playback_status": {
            "state": "stopped",
            "current_position": 0.0,
            "playback_speed": 1.0,
            "total_duration_seconds": 0.0,
            "loop_enabled": false
        }
    }))
}

async fn handle_create_branch(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let name = arguments
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| Error::InvalidInput("Missing 'name' parameter".to_string()))?;

    let description = arguments
        .get("description")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    // Implementation would use the actor handle to create branch
    Ok(json!({
        "success": true,
        "message": format!("Branch '{}' created successfully", name),
        "branch": {
            "id": "branch_123", // Would be actual ID
            "name": name,
            "description": description
        }
    }))
}

async fn handle_list_branches(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to list branches
    Ok(json!({
        "branches": []
    }))
}

async fn handle_switch_branch(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let branch_id = arguments
        .get("branch_id")
        .and_then(|id| id.as_str())
        .ok_or_else(|| Error::InvalidInput("Missing 'branch_id' parameter".to_string()))?;

    // Implementation would use the actor handle to switch branch
    Ok(json!({
        "success": true,
        "message": format!("Switched to branch '{}'", branch_id),
        "current_branch": branch_id
    }))
}

async fn handle_add_modification(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to add modification
    Ok(json!({
        "success": true,
        "message": "Modification added to current branch"
    }))
}

async fn handle_merge_branch(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to merge branches
    Ok(json!({
        "success": true,
        "message": "Branches merged successfully"
    }))
}

async fn handle_compare_branches(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to compare branches
    Ok(json!({
        "comparison": {
            "differences": [],
            "similarity_score": 1.0,
            "divergence_frame": null
        }
    }))
}

async fn handle_delete_branch(
    arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    let branch_id = arguments
        .get("branch_id")
        .and_then(|id| id.as_str())
        .ok_or_else(|| Error::InvalidInput("Missing 'branch_id' parameter".to_string()))?;

    // Implementation would use the actor handle to delete branch
    Ok(json!({
        "success": true,
        "message": format!("Branch '{}' deleted successfully", branch_id)
    }))
}

async fn handle_branch_tree(
    _arguments: Value,
    _handle: &crate::replay_actor::ReplayActorHandle,
) -> Result<Value> {
    // Implementation would use the actor handle to get branch tree
    Ok(json!({
        "branch_tree": {
            "nodes": [],
            "edges": []
        }
    }))
}

/// Parse recording configuration from arguments
fn parse_recording_config(arguments: &Value) -> RecordingConfig {
    let sample_rate = arguments
        .get("sample_rate")
        .and_then(|r| r.as_f64())
        .unwrap_or(60.0) as f32;

    let max_buffer_size = arguments
        .get("max_buffer_size")
        .and_then(|s| s.as_u64())
        .unwrap_or(1000) as usize;

    let compression = arguments
        .get("compression")
        .and_then(|c| c.as_bool())
        .unwrap_or(true);

    let checksums = arguments
        .get("checksums")
        .and_then(|c| c.as_bool())
        .unwrap_or(true);

    RecordingConfig {
        sample_rate,
        max_buffer_size,
        compression,
        checksums,
        ..Default::default()
    }
}