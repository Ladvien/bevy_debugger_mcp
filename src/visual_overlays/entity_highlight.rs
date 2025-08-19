/// Entity Highlighting Overlay Implementation
/// 
/// Provides visual highlighting for entities with customizable colors and highlight modes.
/// This overlay can highlight entities based on various criteria and display them with
/// different visual effects like outlines, glows, or color tints.

use super::{OverlayMetrics, VisualOverlay};
use crate::brp_messages::DebugOverlayType;
#[cfg(feature = "visual_overlays")]
use bevy::prelude::*;
#[cfg(feature = "visual_overlays")]
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
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

/// Custom material for highlighted entities
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct HighlightMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub mode: u32,
    #[uniform(2)]
    pub thickness: f32,
    #[uniform(3)]
    pub intensity: f32,
    #[uniform(4)]
    pub time: f32,
}

impl Material for HighlightMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/highlight_material.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        match self.mode {
            0 => AlphaMode::Blend, // Outline
            1 => AlphaMode::Blend, // Tint
            2 => AlphaMode::Add,   // Glow
            3 => AlphaMode::Blend, // Wireframe
            _ => AlphaMode::Opaque, // Solid
        }
    }
}

impl Default for HighlightMaterial {
    fn default() -> Self {
        Self {
            color: LinearRgba::rgb(1.0, 1.0, 0.0), // Yellow
            mode: 0, // Outline
            thickness: 0.02,
            intensity: 1.0,
            time: 0.0,
        }
    }
}

/// Component to store the original material of a highlighted entity
#[cfg_attr(feature = "visual_overlays", derive(Component))]
#[derive(Debug, Clone)]
pub struct OriginalMaterial<T> {
    pub material: T,
}

/// Component to store highlight material handle
#[cfg_attr(feature = "visual_overlays", derive(Component))]
#[derive(Debug, Clone)]
pub struct HighlightMaterialHandle(pub Handle<HighlightMaterial>);

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
            .add_plugins(MaterialPlugin::<HighlightMaterial>::default())
            .add_systems(Update, (
                update_highlighted_entities,
                animate_highlighted_entities,
                cleanup_old_highlights,
            ))
            .add_systems(PostUpdate, (
                apply_highlight_materials,
                update_highlight_metrics,
            ));
        
        info!("Entity highlight overlay initialized");
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

/// System to update highlighted entities
fn update_highlighted_entities(
    mut commands: Commands,
    config: Res<HighlightConfig>,
    query: Query<Entity, (With<HighlightedEntity>, Without<HighlightMaterialHandle>)>,
    mut materials: ResMut<Assets<HighlightMaterial>>,
) {
    if !config.is_changed() {
        return;
    }
    
    for entity in &query {
        let material = HighlightMaterial {
            color: config.default_color.into(),
            mode: config.default_mode as u32,
            thickness: config.outline_thickness,
            intensity: config.glow_intensity,
            time: 0.0,
        };
        
        let material_handle = materials.add(material);
        commands.entity(entity).insert(HighlightMaterialHandle(material_handle));
    }
}

/// System to animate highlighted entities
fn animate_highlighted_entities(
    time: Res<Time>,
    config: Res<HighlightConfig>,
    mut materials: ResMut<Assets<HighlightMaterial>>,
    query: Query<(&HighlightedEntity, &HighlightMaterialHandle)>,
) {
    let current_time = time.elapsed_secs();
    
    for (highlight, material_handle) in &query {
        if highlight.animated {
            if let Some(material) = materials.get_mut(&material_handle.0) {
                material.time = current_time * config.animation_speed;
            }
        }
    }
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

/// System to apply highlight materials to entities
fn apply_highlight_materials(
    mut commands: Commands,
    config: Res<HighlightConfig>,
    mut materials: ResMut<Assets<HighlightMaterial>>,
    query: Query<(Entity, &HighlightedEntity), Added<HighlightedEntity>>,
) {
    for (entity, highlight) in &query {
        let material = HighlightMaterial {
            color: highlight.color.into(),
            mode: highlight.mode as u32,
            thickness: config.outline_thickness,
            intensity: config.glow_intensity,
            time: 0.0,
        };
        
        let material_handle = materials.add(material);
        commands.entity(entity).insert(HighlightMaterialHandle(material_handle));
    }
}

/// System to update highlight metrics
fn update_highlight_metrics(
    query: Query<&HighlightedEntity>,
) {
    let start_time = Instant::now();
    
    let count = query.iter().count();
    let render_time = start_time.elapsed().as_micros() as u64;
    
    // Estimate memory usage (rough calculation)
    let estimated_memory = count * std::mem::size_of::<HighlightedEntity>() + 
                          count * std::mem::size_of::<HighlightMaterial>() * 2; // Material + handle
    
    let _metrics = OverlayMetrics {
        render_time_us: render_time,
        element_count: count,
        memory_usage_bytes: estimated_memory,
        frame_updates: if count > 0 { 1 } else { 0 },
        active_this_frame: count > 0,
    };
    
    // This would need to be properly implemented to update the specific overlay metrics
    // For now, this is a placeholder showing the concept
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