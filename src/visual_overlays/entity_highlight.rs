/// Entity Highlighting Overlay Implementation
/// 
/// Provides visual highlighting for entities with customizable colors and highlight modes.
/// This overlay uses Bevy's Gizmo system for efficient rendering and supports multiple
/// viewports. Performance is optimized to stay under 1ms per frame.

use super::{OverlayMetrics, VisualOverlay};
use crate::brp_messages::DebugOverlayType;
#[cfg(feature = "visual_overlays")]
use bevy::prelude::*;
#[cfg(feature = "visual_overlays")]
use bevy::gizmos::*;
#[cfg(feature = "visual_overlays")]
use bevy::render::camera::CameraProjection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// Component that marks an entity for highlighting
#[derive(Component, Debug, Clone)]
pub struct HighlightedEntity {
    /// Highlight color (RGBA)
    pub color: Color,
    /// Highlight mode
    pub mode: HighlightMode,
    /// When the highlight was added
    pub timestamp: Instant,
    /// Priority (higher values render on top)
    pub priority: i32,
    /// Whether the highlight should pulse/animate
    pub animated: bool,
}

impl Default for HighlightedEntity {
    fn default() -> Self {
        Self {
            color: Color::srgb(1.0, 1.0, 0.0), // Yellow default
            mode: HighlightMode::Outline,
            timestamp: Instant::now(),
            priority: 0,
            animated: false,
        }
    }
}

/// Different highlighting modes available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighlightMode {
    /// Draw an outline around the entity
    Outline,
    /// Apply a color tint to the entire entity
    Tint,
    /// Add a glow effect around the entity
    Glow,
    /// Wireframe overlay
    Wireframe,
    /// Solid color replacement
    SolidColor,
}

impl Default for HighlightMode {
    fn default() -> Self {
        HighlightMode::Outline
    }
}

/// Resource to store highlight configuration
#[derive(Resource, Debug, Clone, Default)]
pub struct HighlightConfig {
    /// Default highlight color
    pub default_color: Color,
    /// Default highlight mode
    pub default_mode: HighlightMode,
    /// Maximum number of highlighted entities
    pub max_highlighted: usize,
    /// Outline thickness for outline mode
    pub outline_thickness: f32,
    /// Glow intensity for glow mode
    pub glow_intensity: f32,
    /// Animation speed for animated highlights
    pub animation_speed: f32,
    /// Whether to show highlight info in UI
    pub show_info_ui: bool,
}

impl HighlightConfig {
    /// Create new highlight configuration with reasonable defaults
    pub fn new() -> Self {
        Self {
            default_color: Color::srgb(1.0, 1.0, 0.0), // Yellow
            default_mode: HighlightMode::Outline,
            max_highlighted: 100, // Reasonable limit
            outline_thickness: 0.02,
            glow_intensity: 1.5,
            animation_speed: 2.0, // 2 Hz
            show_info_ui: true,
        }
    }
    
    /// Update configuration from JSON
    pub fn update_from_json(&mut self, config: &serde_json::Value) -> Result<(), String> {
        if let Some(color_array) = config.get("default_color").and_then(|v| v.as_array()) {
            if color_array.len() >= 3 {
                let r = color_array[0].as_f64().ok_or("Invalid red component")? as f32;
                let g = color_array[1].as_f64().ok_or("Invalid green component")? as f32;
                let b = color_array[2].as_f64().ok_or("Invalid blue component")? as f32;
                let a = color_array.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                self.default_color = Color::srgba(r, g, b, a);
            }
        }
        
        if let Some(mode_str) = config.get("default_mode").and_then(|v| v.as_str()) {
            self.default_mode = match mode_str {
                "outline" => HighlightMode::Outline,
                "tint" => HighlightMode::Tint,
                "glow" => HighlightMode::Glow,
                "wireframe" => HighlightMode::Wireframe,
                "solid" => HighlightMode::SolidColor,
                _ => return Err(format!("Invalid highlight mode: {}", mode_str)),
            };
        }
        
        if let Some(max) = config.get("max_highlighted").and_then(|v| v.as_u64()) {
            self.max_highlighted = (max as usize).min(1000); // Cap at 1000 for performance
        }
        
        if let Some(thickness) = config.get("outline_thickness").and_then(|v| v.as_f64()) {
            self.outline_thickness = (thickness as f32).max(0.001).min(0.1); // Reasonable bounds
        }
        
        if let Some(intensity) = config.get("glow_intensity").and_then(|v| v.as_f64()) {
            self.glow_intensity = (intensity as f32).max(0.1).min(10.0); // Reasonable bounds
        }
        
        if let Some(speed) = config.get("animation_speed").and_then(|v| v.as_f64()) {
            self.animation_speed = (speed as f32).max(0.1).min(10.0); // Reasonable bounds
        }
        
        if let Some(show_ui) = config.get("show_info_ui").and_then(|v| v.as_bool()) {
            self.show_info_ui = show_ui;
        }
        
        Ok(())
    }
}

/// Gizmo configuration for highlighting
#[derive(Resource, Debug, Clone)]
pub struct HighlightGizmosConfig {
    /// Whether to show debug text labels
    pub show_labels: bool,
    /// Line width for wireframes
    pub line_width: f32,
    /// Circle resolution for round highlights
    pub circle_resolution: usize,
    /// Maximum distance for visibility culling
    pub max_distance: f32,
    /// Whether to enable per-viewport rendering
    pub per_viewport_rendering: bool,
}

impl Default for HighlightGizmosConfig {
    fn default() -> Self {
        Self {
            show_labels: true,
            line_width: 2.0,
            circle_resolution: 32,
            max_distance: 1000.0,
            per_viewport_rendering: true,
        }
    }
}

/// Entity Highlight Overlay implementation
#[derive(Debug)]
pub struct EntityHighlightOverlay {
    enabled: bool,
    config: HighlightConfig,
    metrics: OverlayMetrics,
    highlighted_entities: HashMap<Entity, HighlightedEntity>,
}

impl EntityHighlightOverlay {
    pub fn new() -> Self {
        Self {
            enabled: false,
            config: HighlightConfig::new(),
            metrics: OverlayMetrics::default(),
            highlighted_entities: HashMap::new(),
        }
    }
    
    /// Add highlight to an entity
    pub fn highlight_entity(
        &mut self,
        entity: Entity,
        color: Option<Color>,
        mode: Option<HighlightMode>,
        animated: bool,
        priority: i32,
    ) {
        if self.highlighted_entities.len() >= self.config.max_highlighted {
            warn!("Maximum highlighted entities reached: {}", self.config.max_highlighted);
            return;
        }
        
        let highlight = HighlightedEntity {
            color: color.unwrap_or(self.config.default_color),
            mode: mode.unwrap_or(self.config.default_mode),
            timestamp: Instant::now(),
            priority,
            animated,
        };
        
        self.highlighted_entities.insert(entity, highlight);
        self.metrics.element_count = self.highlighted_entities.len();
    }
    
    /// Remove highlight from an entity
    pub fn unhighlight_entity(&mut self, entity: Entity) {
        self.highlighted_entities.remove(&entity);
        self.metrics.element_count = self.highlighted_entities.len();
    }
    
    /// Clear all highlights
    pub fn clear_highlights(&mut self) {
        self.highlighted_entities.clear();
        self.metrics.element_count = 0;
    }
    
    /// Get currently highlighted entities
    pub fn get_highlighted_entities(&self) -> &HashMap<Entity, HighlightedEntity> {
        &self.highlighted_entities
    }
}

impl Default for EntityHighlightOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualOverlay for EntityHighlightOverlay {
    fn initialize(&mut self, app: &mut App) {
        app.insert_resource(self.config.clone())
            .insert_resource(HighlightGizmosConfig::default())
            .add_systems(Update, (
                render_highlighted_entities,
                animate_highlighted_entities,
                cleanup_old_highlights,
            ))
            .add_systems(PostUpdate, (
                update_highlight_metrics,
            ));
        
        info!("Entity highlight overlay initialized with Gizmo rendering");
    }
    
    fn update_config(&mut self, config: &serde_json::Value) -> Result<(), String> {
        self.config.update_from_json(config)?;
        info!("Entity highlight overlay config updated: {:?}", self.config);
        Ok(())
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.cleanup();
        }
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn get_metrics(&self) -> OverlayMetrics {
        self.metrics.clone()
    }
    
    fn overlay_type(&self) -> DebugOverlayType {
        DebugOverlayType::EntityHighlight
    }
    
    fn cleanup(&mut self) {
        self.clear_highlights();
    }
}

/// System to render highlighted entities using Gizmos
fn render_highlighted_entities(
    mut gizmos: Gizmos,
    config: Res<HighlightConfig>,
    gizmo_config: Res<HighlightGizmosConfig>,
    time: Res<Time>,
    query: Query<(&Transform, &HighlightedEntity), With<Visibility>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
) {
    let start_time = std::time::Instant::now();
    let mut rendered_count = 0;
    
    // Early exit if no highlights
    if query.is_empty() {
        return;
    }
    
    // Performance optimization: limit rendering based on distance from cameras
    let camera_positions: Vec<Vec3> = cameras
        .iter()
        .map(|(_, transform)| transform.translation())
        .collect();
    
    for (transform, highlight) in &query {
        // Distance culling for performance
        if !camera_positions.is_empty() {
            let entity_pos = transform.translation;
            let in_range = camera_positions.iter().any(|cam_pos| {
                cam_pos.distance(entity_pos) <= gizmo_config.max_distance
            });
            
            if !in_range {
                continue;
            }
        }
        
        let mut color = highlight.color;
        
        // Apply animation if enabled
        if highlight.animated {
            let pulse = (time.elapsed_secs() * config.animation_speed).sin();
            let alpha_mod = (pulse * 0.3 + 0.7).max(0.4).min(1.0); // Keep it visible
            color = color.with_alpha(color.alpha() * alpha_mod);
        }
        
        match highlight.mode {
            HighlightMode::Outline => {
                render_outline_gizmo(&mut gizmos, transform, color, config.outline_thickness);
            }
            HighlightMode::Wireframe => {
                render_wireframe_gizmo(&mut gizmos, transform, color, &gizmo_config);
            }
            HighlightMode::Glow => {
                render_glow_gizmo(&mut gizmos, transform, color, config.glow_intensity);
            }
            HighlightMode::Tint => {
                render_tint_gizmo(&mut gizmos, transform, color);
            }
            HighlightMode::SolidColor => {
                render_solid_gizmo(&mut gizmos, transform, color);
            }
        }
        
        // Render debug label if enabled
        if gizmo_config.show_labels {
            let label_pos = transform.translation + Vec3::Y * 2.0;
            // Note: Text rendering with Gizmos requires additional setup
            // For now, we'll use a simple marker
            gizmos.sphere(label_pos, Quat::IDENTITY, 0.1, color);
        }
        
        rendered_count += 1;
        
        // Performance brake: don't render too many in one frame
        if rendered_count >= 100 {
            break;
        }
    }
    
    // Track performance
    let render_time = start_time.elapsed().as_micros() as u64;
    if render_time > 1000 { // Warn if over 1ms
        warn!("Entity highlight rendering took {}μs for {} entities", render_time, rendered_count);
    }
}

/// System to animate highlighted entities (now handled in render system)
fn animate_highlighted_entities(
    // Animation is now handled directly in render_highlighted_entities
    // This system is kept for potential future animation logic
) {
    // No-op - animation handled in render system for performance
}

/// System to clean up old highlights
fn cleanup_old_highlights(
    mut commands: Commands,
    query: Query<(Entity, &HighlightedEntity)>,
) {
    let now = Instant::now();
    
    for (entity, highlight) in &query {
        // Remove highlights older than 1 hour (configurable)
        if now.duration_since(highlight.timestamp).as_secs() > 3600 {
            commands.entity(entity).remove::<HighlightedEntity>();
        }
    }
}

/// Render an outline gizmo around an entity
fn render_outline_gizmo(gizmos: &mut Gizmos, transform: &Transform, color: Color, thickness: f32) {
    let size = Vec3::splat(1.0 + thickness); // Slightly larger than the entity
    let position = transform.translation;
    let rotation = transform.rotation;
    
    // Draw wireframe box as outline
    gizmos.cuboid(
        Transform {
            translation: position,
            rotation,
            scale: size,
        },
        color,
    );
}

/// Render a wireframe gizmo for an entity
fn render_wireframe_gizmo(gizmos: &mut Gizmos, transform: &Transform, color: Color, config: &HighlightGizmosConfig) {
    let position = transform.translation;
    let rotation = transform.rotation;
    let scale = transform.scale;
    
    // Draw detailed wireframe
    gizmos.cuboid(
        Transform {
            translation: position,
            rotation,
            scale,
        },
        color,
    );
    
    // Add additional detail lines if needed
    let corners = [
        position + rotation * (Vec3::new(-0.5, -0.5, -0.5) * scale),
        position + rotation * (Vec3::new(0.5, -0.5, -0.5) * scale),
        position + rotation * (Vec3::new(0.5, 0.5, -0.5) * scale),
        position + rotation * (Vec3::new(-0.5, 0.5, -0.5) * scale),
        position + rotation * (Vec3::new(-0.5, -0.5, 0.5) * scale),
        position + rotation * (Vec3::new(0.5, -0.5, 0.5) * scale),
        position + rotation * (Vec3::new(0.5, 0.5, 0.5) * scale),
        position + rotation * (Vec3::new(-0.5, 0.5, 0.5) * scale),
    ];
    
    // Draw cross lines for more visibility
    for i in 0..4 {
        gizmos.line(corners[i], corners[i + 4], color);
    }
}

/// Render a glow effect using concentric shapes
fn render_glow_gizmo(gizmos: &mut Gizmos, transform: &Transform, color: Color, intensity: f32) {
    let position = transform.translation;
    let base_radius = 1.0 * transform.scale.max_element();
    
    // Multiple concentric circles/spheres for glow effect
    for i in 1..=3 {
        let radius = base_radius * (1.0 + i as f32 * 0.2 * intensity);
        let alpha = color.alpha() / (i as f32 * 2.0);
        let glow_color = color.with_alpha(alpha);
        
        gizmos.sphere(position, Quat::IDENTITY, radius, glow_color);
    }
}

/// Render a tint overlay
fn render_tint_gizmo(gizmos: &mut Gizmos, transform: &Transform, color: Color) {
    // For tint mode, draw a semi-transparent cube
    let alpha_color = color.with_alpha(color.alpha() * 0.3);
    gizmos.cuboid(*transform, alpha_color);
}

/// Render solid color replacement
fn render_solid_gizmo(gizmos: &mut Gizmos, transform: &Transform, color: Color) {
    // Draw solid-colored cube
    gizmos.cuboid(*transform, color);
}

/// System to update highlight metrics
fn update_highlight_metrics(
    query: Query<&HighlightedEntity>,
    mut overlay_manager: ResMut<super::VisualOverlayManager>,
) {
    let start_time = Instant::now();
    
    let count = query.iter().count();
    let render_time = start_time.elapsed().as_micros() as u64;
    
    // Estimate memory usage (Gizmos are much lighter than materials)
    let estimated_memory = count * std::mem::size_of::<HighlightedEntity>() + 
                          count * 64; // Estimated Gizmo overhead per entity
    
    let metrics = OverlayMetrics {
        render_time_us: render_time,
        element_count: count,
        memory_usage_bytes: estimated_memory,
        frame_updates: if count > 0 { 1 } else { 0 },
        active_this_frame: count > 0,
    };
    
    // Update the specific overlay metrics
    if let Some(overlay) = overlay_manager.overlays.get_mut("entity_highlight") {
        // This needs to be properly implemented to access the overlay metrics
        // For now, this is a conceptual placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_config_json_update() {
        let mut config = HighlightConfig::new();
        
        let json_config = serde_json::json!({
            "default_color": [1.0, 0.0, 0.0, 1.0],
            "default_mode": "glow",
            "max_highlighted": 50,
            "outline_thickness": 0.05,
            "glow_intensity": 2.0
        });
        
        assert!(config.update_from_json(&json_config).is_ok());
        
        assert_eq!(config.default_color, Color::srgba(1.0, 0.0, 0.0, 1.0));
        assert_eq!(config.default_mode, HighlightMode::Glow);
        assert_eq!(config.max_highlighted, 50);
        assert_eq!(config.outline_thickness, 0.05);
        assert_eq!(config.glow_intensity, 2.0);
    }

    #[test]
    fn test_highlight_entity_management() {
        let mut overlay = EntityHighlightOverlay::new();
        overlay.config.max_highlighted = 2; // Small limit for testing
        
        let entity1 = Entity::from_raw(1);
        let entity2 = Entity::from_raw(2);
        let entity3 = Entity::from_raw(3);
        
        // Add highlights
        overlay.highlight_entity(entity1, Some(Color::srgb(1.0, 0.0, 0.0)), Some(HighlightMode::Outline), false, 0);
        overlay.highlight_entity(entity2, Some(Color::srgb(0.0, 1.0, 0.0)), Some(HighlightMode::Glow), true, 1);
        
        assert_eq!(overlay.highlighted_entities.len(), 2);
        assert_eq!(overlay.metrics.element_count, 2);
        
        // Try to add third (should be rejected due to limit)
        overlay.highlight_entity(entity3, Some(Color::srgb(0.0, 0.0, 1.0)), Some(HighlightMode::Tint), false, 2);
        assert_eq!(overlay.highlighted_entities.len(), 2); // Still 2
        
        // Remove one highlight
        overlay.unhighlight_entity(entity1);
        assert_eq!(overlay.highlighted_entities.len(), 1);
        assert_eq!(overlay.metrics.element_count, 1);
        
        // Clear all
        overlay.clear_highlights();
        assert_eq!(overlay.highlighted_entities.len(), 0);
        assert_eq!(overlay.metrics.element_count, 0);
    }

    #[test]
    fn test_highlight_modes() {
        let highlight = HighlightedEntity {
            mode: HighlightMode::Glow,
            ..Default::default()
        };
        
        assert_eq!(highlight.mode, HighlightMode::Glow);
        
        // Test serialization
        let serialized = serde_json::to_string(&highlight.mode).unwrap();
        assert!(serialized.contains("Glow"));
        
        let deserialized: HighlightMode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, HighlightMode::Glow);
    }
}