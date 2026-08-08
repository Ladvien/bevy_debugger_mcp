//! Screenshot capture with region cropping and zoom.
//!
//! The handler spawns a [`Screenshot`] of the primary window and observes its completion. On capture
//! it converts the GPU image, optionally crops to a region, optionally scales it, and writes a PNG.
//!
//! # The capture is asynchronous, and the response says so
//!
//! `Screenshot` completes some frames after it is spawned, so this handler returns *before* the image
//! exists. That is why it always writes to a **path** and reports that path back, rather than
//! returning PNG bytes: there is nothing to return yet at the moment the BRP method replies. A caller
//! reads the file once it appears.
//!
//! An earlier version of this doc promised "PNG bytes are returned if no path is given". It could not
//! and did not — the bytes were discarded. Defaulting the path is the honest version of that contract.

use bevy::prelude::*;
use bevy::remote::{error_codes, BrpError, BrpResult};
use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured};
use image::{imageops::FilterType, ImageFormat};
use serde::Deserialize;
use serde_json::{json, Value};

/// Where a capture goes when the caller does not name a path.
const DEFAULT_PATH: &str = "./screenshot.png";

/// Parameters for the `bevy_debugger/screenshot` BRP method.
#[derive(Debug, Deserialize)]
pub struct ScreenshotParams {
    /// Path to write the PNG to. Defaults to `./screenshot.png`, relative to the game's working
    /// directory — not the agent's.
    pub path: Option<String>,
    /// Region to crop, in physical pixels: `{ x, y, width, height }`. Clamped to the captured image;
    /// a region entirely outside it is an error rather than an empty file.
    pub region: Option<Region>,
    /// Scale factor applied after cropping. `1.0` leaves the image alone.
    #[serde(default = "one")]
    pub zoom: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Build a `BrpError` for a malformed parameter payload.
fn invalid_params(message: String) -> BrpError {
    BrpError { code: error_codes::INVALID_PARAMS, message, data: None }
}

/// BRP handler: `bevy_debugger/screenshot`.
pub fn handle_screenshot(In(params): In<Option<Value>>, mut commands: Commands) -> BrpResult {
    let params: ScreenshotParams = match params.as_ref() {
        Some(p) => serde_json::from_value(p.clone())
            .map_err(|e| invalid_params(format!("invalid screenshot params: {e}")))?,
        None => ScreenshotParams { path: None, region: None, zoom: 1.0 },
    };

    if !params.zoom.is_finite() || params.zoom <= 0.0 {
        return Err(invalid_params(format!(
            "zoom must be finite and greater than zero, got {}",
            params.zoom
        )));
    }
    // Written as a nested `if` rather than a let-chain: this crate is edition 2021, where let-chains
    // are not available regardless of compiler version.
    if let Some(r) = &params.region {
        if r.width == 0 || r.height == 0 {
            return Err(invalid_params(
                "region width and height must both be non-zero".to_string(),
            ));
        }
    }

    let path = params.path.unwrap_or_else(|| DEFAULT_PATH.to_string());
    let reported = path.clone();
    let region = params.region;
    let zoom = params.zoom;

    commands.spawn(Screenshot::primary_window()).observe(
        move |trigger: On<ScreenshotCaptured>| {
            if let Err(e) = write_capture(&trigger.image, region.as_ref(), zoom, &path) {
                error!("bevy_debugger/screenshot: {e}");
            } else {
                info!("bevy_debugger/screenshot: wrote {path}");
            }
        },
    );

    Ok(json!({
        "success": true,
        "path": reported,
        "message": "capture initiated; the file appears once the frame completes",
    }))
}

/// Convert, crop, scale and encode. Split out so every failure has one place to be reported from, and
/// so the observer closure stays readable.
fn write_capture(
    image: &Image,
    region: Option<&Region>,
    zoom: f32,
    path: &str,
) -> Result<(), String> {
    let dynamic = image
        .clone()
        .try_into_dynamic()
        .map_err(|e| format!("the captured image could not be converted: {e}"))?;

    // Drop the alpha channel. With HDR enabled it carries brightness rather than opacity, so keeping
    // it makes the PNG look wrong — `bevy_render`'s own `save_to_disk` does exactly this.
    let mut rgb = dynamic.to_rgb8();

    if let Some(r) = region {
        let (w, h) = (rgb.width(), rgb.height());
        if r.x >= w || r.y >= h {
            return Err(format!(
                "region origin ({}, {}) is outside the {w}x{h} capture",
                r.x, r.y
            ));
        }
        // Clamp rather than refuse: a region running off the edge is a reasonable request, an origin
        // off the edge is a mistake.
        let width = r.width.min(w - r.x);
        let height = r.height.min(h - r.y);
        rgb = image::imageops::crop_imm(&rgb, r.x, r.y, width, height).to_image();
    }

    if zoom != 1.0 {
        let width = ((rgb.width() as f32) * zoom).round().max(1.0) as u32;
        let height = ((rgb.height() as f32) * zoom).round().max(1.0) as u32;
        // Lanczos3 for downscales, which is what a zoom < 1 mostly is; it is also acceptable
        // magnifying, and one filter beats a branch nobody tuned.
        rgb = image::imageops::resize(&rgb, width, height, FilterType::Lanczos3);
    }

    rgb.save_with_format(path, ImageFormat::Png)
        .map_err(|e| format!("could not write {path}: {e}"))
}
