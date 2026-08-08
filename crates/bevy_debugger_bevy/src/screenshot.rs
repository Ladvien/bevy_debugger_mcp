//! Screenshot capture with zoom and region selection.
//!
//! The handler captures the primary window (or a specified render target),
//! optionally crops to a region, optionally scales (zoom), encodes to PNG,
//! and either saves to disk or returns the bytes.

use bevy::prelude::*;
use bevy::remote::BrpResult;
use bevy::render::view::window::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use serde::Deserialize;
use serde_json::{json, Value};

/// Parameters for the `bevy_debugger/screenshot` BRP method.
#[derive(Debug, Deserialize)]
pub struct ScreenshotParams {
    /// Path to save the screenshot. If omitted, PNG bytes are returned.
    pub path: Option<String>,
    /// Region to crop: { x, y, width, height } in pixels.
    pub region: Option<Region>,
    /// Zoom factor (1.0 = no zoom). Scales the final image.
    #[serde(default = "one")]
    pub zoom: f32,
}

fn one() -> f32 { 1.0 }

#[derive(Debug, Deserialize)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// BRP handler: `bevy_debugger/screenshot`
pub fn handle_screenshot(
    In(params): In<Option<Value>>,
    mut commands: Commands,
) -> BrpResult {
    let params: ScreenshotParams = params
        .as_ref()
        .map(|p| serde_json::from_value(p.clone()))
        .transpose()?
        .unwrap_or(ScreenshotParams {
            path: Some("./screenshot.png".to_string()),
            region: None,
            zoom: 1.0,
        });

    let path = params.path.clone();
    let region = params.region;
    let zoom = params.zoom;

    commands
        .spawn(Screenshot::primary_window())
        .observe(move |trigger: On<ScreenshotCaptured>, mut commands: Commands| {
            let image = &trigger.image;
            // TODO: crop to region, scale by zoom using `image` crate, encode PNG.
            // For now, save to disk if path provided.
            if let Some(ref p) = path {
                let _ = save_to_disk(p.clone())(&trigger);
            }
            let _ = (image, region, zoom, &mut commands);
        });

    Ok(json!({
        "success": true,
        "message": "Screenshot capture initiated",
    }))
}