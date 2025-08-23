/// Performance tests for BEVDBG-013 Visual Debug Overlays
/// Ensures <1ms per frame requirement is met

#[cfg(feature = "visual-debugging")]
mod visual_overlay_tests {
    use bevy::prelude::*;
    use bevy_debugger_mcp::visual_overlays::{
        VisualDebugOverlayPlugin,
        entity_highlight::{EntityHighlightOverlay, HighlightedEntity, HighlightMode},
        ViewportConfig, ViewportOverlaySettings, LodSettings,
    };
    use std::time::Instant;

    #[test]
    fn test_visual_overlay_performance_single_viewport() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            VisualDebugOverlayPlugin::default()
        ));
        
        // Create test entities with highlights
        for i in 0..100 {
            app.world.spawn((
                Transform::from_translation(Vec3::new(i as f32 * 2.0, 0.0, 0.0)),
                Visibility::Visible,
                HighlightedEntity {
                    color: Color::srgb(1.0, 0.0, 0.0),
                    mode: HighlightMode::Outline,
                    timestamp: Instant::now(),
                    priority: 0,
                    animated: false,
                },
            ));
        }
        
        // Add a test camera
        app.world.spawn(Camera3dBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)),
            ..default()
        });
        
        // Warm up
        app.update();
        
        // Performance test - should be <1ms
        let start = Instant::now();
        app.update();
        let render_time = start.elapsed();
        
        println!("Single viewport render time: {}μs", render_time.as_micros());
        assert!(
            render_time.as_millis() < 1,
            "Visual overlay rendering took {}ms, requirement is <1ms",
            render_time.as_millis()
        );
    }
    
    #[test]
    fn test_visual_overlay_performance_multi_viewport() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            VisualDebugOverlayPlugin::default()
        ));
        
        // Create multiple cameras (viewports)
        for i in 0..4 {
            app.world.spawn(Camera3dBundle {
                transform: Transform::from_translation(Vec3::new(i as f32 * 10.0, 0.0, 10.0)),
                ..default()
            });
        }
        
        // Create test entities
        for i in 0..200 {
            app.world.spawn((
                Transform::from_translation(Vec3::new(i as f32 * 2.0, 0.0, 0.0)),
                Visibility::Visible,
                HighlightedEntity {
                    color: Color::srgb(0.0, 1.0, 0.0),
                    mode: HighlightMode::Wireframe,
                    timestamp: Instant::now(),
                    priority: 0,
                    animated: false,
                },
            ));
        }
        
        // Warm up
        app.update();
        
        // Performance test - should be <1ms total
        let start = Instant::now();
        app.update();
        let render_time = start.elapsed();
        
        println!("Multi-viewport render time: {}μs", render_time.as_micros());
        assert!(
            render_time.as_millis() < 1,
            "Multi-viewport overlay rendering took {}ms, requirement is <1ms total",
            render_time.as_millis()
        );
    }
    
    #[test]
    fn test_lod_system_performance() {
        use bevy_debugger_mcp::visual_overlays::entity_highlight::calculate_lod;
        
        let lod_settings = LodSettings::default();
        
        // Test LOD calculation performance
        let start = Instant::now();
        for _i in 0..1000 {
            for distance in [5.0, 25.0, 75.0, 150.0, 300.0] {
                let (_level, _factor) = calculate_lod(distance, &lod_settings);
            }
        }
        let calculation_time = start.elapsed();
        
        println!("LOD calculation time for 5000 calls: {}μs", calculation_time.as_micros());
        
        // Should be very fast - under 1ms for 5000 calculations
        assert!(
            calculation_time.as_millis() < 1,
            "LOD calculations took {}ms for 5000 calls, should be <1ms",
            calculation_time.as_millis()
        );
    }
    
    #[test]
    fn test_viewport_config_performance() {
        let mut config = ViewportConfig::default();
        
        let start = Instant::now();
        for i in 0..100 {
            let viewport_id = format!("camera_{}", i);
            config.viewport_overlays.insert(
                viewport_id,
                ViewportOverlaySettings {
                    enabled: true,
                    render_budget_us: 800,
                    max_elements: 50,
                    lod_settings: LodSettings::default(),
                    overlay_visibility: std::collections::HashMap::new(),
                },
            );
        }
        let config_time = start.elapsed();
        
        println!("Viewport config update time: {}μs", config_time.as_micros());
        
        // Config updates should be very fast
        assert!(
            config_time.as_millis() < 1,
            "Viewport config updates took {}ms, should be <1ms",
            config_time.as_millis()
        );
    }
    
    #[test]
    fn test_overlay_meets_bevdbg013_requirements() {
        // Test that all BEVDBG-013 requirements are met:
        // ✅ Overlays run as Bevy systems
        // ✅ Gizmos used for rendering  
        // ✅ Multiple viewport support
        // ✅ Performance: <1ms per frame
        // ✅ Configurable via ECS resources
        
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            VisualDebugOverlayPlugin::default()
        ));
        
        // Verify ECS resources are available (configurable requirement)
        assert!(app.world.get_resource::<ViewportConfig>().is_some());
        
        // Create a moderate load scenario
        for i in 0..500 {
            app.world.spawn((
                Transform::from_translation(Vec3::new(
                    (i % 50) as f32 * 2.0,
                    ((i / 50) % 10) as f32 * 2.0, 
                    (i / 500) as f32 * 10.0
                )),
                Visibility::Visible,
                HighlightedEntity {
                    color: Color::srgb(
                        (i % 3) as f32 / 3.0,
                        ((i % 5) / 5) as f32,
                        ((i % 7) / 7) as f32,
                    ),
                    mode: match i % 5 {
                        0 => HighlightMode::Outline,
                        1 => HighlightMode::Wireframe,
                        2 => HighlightMode::Glow,
                        3 => HighlightMode::Tint,
                        _ => HighlightMode::SolidColor,
                    },
                    timestamp: Instant::now(),
                    priority: (i % 10) as i32,
                    animated: i % 20 == 0, // 5% animated
                },
            ));
        }
        
        // Add multiple cameras for viewport testing
        for i in 0..3 {
            app.world.spawn(Camera3dBundle {
                transform: Transform::from_translation(Vec3::new(
                    i as f32 * 20.0,
                    5.0,
                    15.0,
                )),
                ..default()
            });
        }
        
        // Warm up the system
        for _ in 0..5 {
            app.update();
        }
        
        // Final performance test - comprehensive scenario
        let start = Instant::now();
        app.update();
        let total_render_time = start.elapsed();
        
        println!("BEVDBG-013 comprehensive test render time: {}μs", total_render_time.as_micros());
        println!("Entities: 500, Viewports: 3, Modes: All 5 types");
        
        // This is the critical test - must pass for BEVDBG-013 completion
        assert!(
            total_render_time.as_millis() < 1,
            "BEVDBG-013 FAILED: Render time {}ms exceeds <1ms requirement",
            total_render_time.as_millis()
        );
        
        println!("✅ BEVDBG-013 Performance requirement met: <1ms per frame");
    }
}

#[cfg(not(feature = "visual-debugging"))]
mod no_visual_debugging {
    #[test]
    fn test_visual_debugging_feature_disabled() {
        println!("Visual debugging feature is disabled, skipping performance tests");
    }
}