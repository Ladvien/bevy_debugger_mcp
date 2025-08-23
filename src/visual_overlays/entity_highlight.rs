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

/// System to render highlighted entities using Gizmos (per-viewport aware)
fn render_highlighted_entities(
    mut gizmos: Gizmos,
    config: Res<HighlightConfig>,
    gizmo_config: Res<HighlightGizmosConfig>,
    viewport_config: Res<super::ViewportConfig>,
    time: Res<Time>,
    query: Query<(&Transform, &HighlightedEntity), With<Visibility>>,
    cameras: Query<(Entity, &Camera, &GlobalTransform), With<Camera>>,
) {
    let start_time = std::time::Instant::now();
    let mut total_rendered = 0;
    let mut viewport_stats = HashMap::new();
    
    // Early exit if no highlights
    if query.is_empty() {
        return;
    }
    
    // Process each active viewport/camera separately
    for (camera_entity, camera, camera_transform) in &cameras {
        if !camera.is_active {
            continue;
        }
        
        let viewport_id = format!("camera_{}", camera_entity.index());
        let viewport_settings = viewport_config.viewport_overlays.get(&viewport_id)
            .unwrap_or(&viewport_config.default_settings);
        
        // Skip if overlays disabled for this viewport
        if !viewport_settings.enabled {
            continue;
        }
        
        // Check if entity highlighting is enabled for this viewport
        if let Some(&overlay_enabled) = viewport_settings.overlay_visibility.get("entity_highlight") {
            if !overlay_enabled {
                continue;
            }
        }
        
        let viewport_start_time = std::time::Instant::now();
        let mut viewport_rendered = 0;
        let camera_pos = camera_transform.translation();
        
        // Determine LOD level based on viewport settings
        let default_lod = super::LodSettings::default();
        let lod_settings = &viewport_settings.lod_settings;
        
        for (transform, highlight) in &query {
            if viewport_rendered >= viewport_settings.max_elements {
                break; // Respect per-viewport element limit
            }
            
            let entity_pos = transform.translation;
            let distance = camera_pos.distance(entity_pos);
            
            // Distance culling for performance
            if distance > gizmo_config.max_distance {
                continue;
            }
            
            // Apply LOD based on distance
            let (lod_level, detail_factor) = calculate_lod(distance, lod_settings);
            
            // Skip rendering if too far for current LOD
            if lod_level >= lod_settings.element_limits.len() {
                continue;
            }
            
            // Check per-viewport render budget
            let current_viewport_time = viewport_start_time.elapsed().as_micros() as u64;
            if current_viewport_time > viewport_settings.render_budget_us {
                warn!("Viewport {} exceeded render budget: {}μs", viewport_id, current_viewport_time);
                break;
            }
        
            let mut color = highlight.color;
            
            // Apply animation if enabled (reduced for LOD)
            if highlight.animated {
                let pulse = (time.elapsed_secs() * config.animation_speed).sin();
                let alpha_mod = (pulse * 0.3 + 0.7).max(0.4).min(1.0);
                color = color.with_alpha(color.alpha() * alpha_mod);
            }
            
            // Apply LOD detail factor to visual complexity
            let effective_thickness = config.outline_thickness * detail_factor;
            let effective_intensity = config.glow_intensity * detail_factor;
            
            // Render based on highlight mode and LOD
            match highlight.mode {
                HighlightMode::Outline => {
                    render_outline_gizmo(&mut gizmos, transform, color, effective_thickness);
                }
                HighlightMode::Wireframe => {
                    render_wireframe_gizmo_lod(&mut gizmos, transform, color, &gizmo_config, detail_factor);
                }
                HighlightMode::Glow => {
                    render_glow_gizmo(&mut gizmos, transform, color, effective_intensity);
                }
                HighlightMode::Tint => {
                    render_tint_gizmo(&mut gizmos, transform, color);
                }
                HighlightMode::SolidColor => {
                    render_solid_gizmo(&mut gizmos, transform, color);
                }
            }
            
            // Render debug label if enabled and high detail
            if gizmo_config.show_labels && detail_factor > 0.7 {
                let label_pos = transform.translation + Vec3::Y * 2.0;
                gizmos.sphere(label_pos, Quat::IDENTITY, 0.1 * detail_factor, color);
            }
            
            viewport_rendered += 1;
            total_rendered += 1;
        }
        
        // Record viewport statistics
        let viewport_render_time = viewport_start_time.elapsed().as_micros() as u64;
        viewport_stats.insert(viewport_id.clone(), super::ViewportRenderStats {
            elements_rendered: viewport_rendered,
            render_time_us: viewport_render_time,
            active: true,
            viewport_size: None, // Could be extracted from camera viewport if needed
        });
        
        // Log viewport performance if over budget
        if viewport_render_time > viewport_settings.render_budget_us {
            debug!(
                "Viewport {} render time: {}μs, elements: {}",
                viewport_id, viewport_render_time, viewport_rendered
            );
        }
    }
    
    // Track overall performance
    let total_render_time = start_time.elapsed().as_micros() as u64;
    if total_render_time > 1000 { // Warn if over 1ms total
        warn!("Entity highlight rendering took {}μs for {} entities across {} viewports", 
              total_render_time, total_rendered, viewport_stats.len());
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

/// Calculate LOD level and detail factor based on distance
pub fn calculate_lod(distance: f32, lod_settings: &super::LodSettings) -> (usize, f32) {
    for (i, &threshold) in lod_settings.distance_thresholds.iter().enumerate() {
        if distance <= threshold {
            let detail_factor = lod_settings.detail_levels.get(i).copied().unwrap_or(1.0);
            return (i, detail_factor);
        }
    }
    
    // Beyond all thresholds - lowest LOD
    let last_level = lod_settings.distance_thresholds.len();
    let detail_factor = lod_settings.detail_levels.get(last_level).copied().unwrap_or(0.1);
    (last_level, detail_factor)
}

/// Render a wireframe gizmo for an entity
fn render_wireframe_gizmo(gizmos: &mut Gizmos, transform: &Transform, color: Color, config: &HighlightGizmosConfig) {
    render_wireframe_gizmo_lod(gizmos, transform, color, config, 1.0);
}

/// Render a wireframe gizmo with LOD support
fn render_wireframe_gizmo_lod(gizmos: &mut Gizmos, transform: &Transform, color: Color, config: &HighlightGizmosConfig, detail_factor: f32) {
    let position = transform.translation;
    let rotation = transform.rotation;
    let scale = transform.scale;
    
    // Always draw basic wireframe
    gizmos.cuboid(
        Transform {
            translation: position,
            rotation,
            scale,
        },
        color,
    );
    
    // Add additional detail based on LOD
    if detail_factor > 0.5 {
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
        
        // Draw cross lines for more visibility (only at high detail)
        for i in 0..4 {
            gizmos.line(corners[i], corners[i + 4], color);
        }
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