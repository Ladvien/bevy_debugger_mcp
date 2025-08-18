use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::playback_system::*;
use bevy_debugger_mcp::recording_system::{Frame, Recording, RecordingConfig};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_playback_controller_lifecycle() {
    let controller = PlaybackController::new(Box::new(DirectSync));

    // Initial state should be stopped
    assert_eq!(controller.get_state().await, PlaybackState::Stopped);

    // Create a test recording
    let recording = Recording {
        config: RecordingConfig::default(),
        frames: vec![
            Frame {
                frame_number: 0,
                timestamp: Duration::from_secs(0),
                entities: HashMap::new(),
                events: Vec::new(),
                checksum: None,
            },
            Frame {
                frame_number: 1,
                timestamp: Duration::from_secs(1),
                entities: HashMap::new(),
                events: Vec::new(),
                checksum: None,
            },
        ],
        delta_frames: Vec::new(),
        markers: Vec::new(),
        total_frames: 2,
        duration: Duration::from_secs(2),
        version: RecordingVersion::current(),
    };

    // Load recording
    controller.load_recording(recording).await.unwrap();
    assert_eq!(controller.get_state().await, PlaybackState::Stopped);

    // Try to pause when not playing (should fail)
    assert!(controller.pause().await.is_err());

    // Seek to frame
    controller.seek_to_frame(1).await.unwrap();
    assert_eq!(controller.get_state().await, PlaybackState::Paused);

    // Stop playback
    controller.stop().await.unwrap();
    assert_eq!(controller.get_state().await, PlaybackState::Stopped);
}

#[tokio::test]
async fn test_playback_speed_validation() {
    // Valid speeds
    assert!(PlaybackSpeed::new(0.5).is_ok());
    assert!(PlaybackSpeed::new(1.0).is_ok());
    assert!(PlaybackSpeed::new(2.0).is_ok());
    assert!(PlaybackSpeed::new(10.0).is_ok());

    // Invalid speeds
    assert!(PlaybackSpeed::new(0.0).is_err());
    assert!(PlaybackSpeed::new(-1.0).is_err());
    assert!(PlaybackSpeed::new(101.0).is_err());

    // Default speed
    let default_speed = PlaybackSpeed::default();
    assert_eq!(default_speed.value(), 1.0);
}

#[tokio::test]
async fn test_recording_version_compatibility() {
    let v1_0_0 = RecordingVersion {
        major: 1,
        minor: 0,
        patch: 0,
    };
    let v1_0_1 = RecordingVersion {
        major: 1,
        minor: 0,
        patch: 1,
    };
    let v1_1_0 = RecordingVersion {
        major: 1,
        minor: 1,
        patch: 0,
    };
    let v2_0_0 = RecordingVersion {
        major: 2,
        minor: 0,
        patch: 0,
    };

    // Same version
    assert!(v1_0_0.is_compatible(&v1_0_0));

    // Patch version difference (compatible)
    assert!(v1_0_0.is_compatible(&v1_0_1));

    // Minor version too high (incompatible)
    assert!(!v1_0_0.is_compatible(&v1_1_0));

    // Major version mismatch (incompatible)
    assert!(!v1_0_0.is_compatible(&v2_0_0));

    // Higher minor version can read lower
    assert!(v1_1_0.is_compatible(&v1_0_0));
}

#[tokio::test]
async fn test_frame_interpolation() {
    let mut interpolator = FrameInterpolator::new();

    let frame1 = Frame {
        frame_number: 0,
        timestamp: Duration::from_secs(0),
        entities: HashMap::new(),
        events: Vec::new(),
        checksum: None,
    };

    let frame2 = Frame {
        frame_number: 10,
        timestamp: Duration::from_secs(10),
        entities: HashMap::new(),
        events: Vec::new(),
        checksum: None,
    };

    interpolator.set_frames(frame1, frame2);

    // Test interpolation at different points
    let interp_0 = interpolator.interpolate(0.0).unwrap();
    assert_eq!(interp_0.timestamp, Duration::from_secs(0));

    let interp_half = interpolator.interpolate(0.5).unwrap();
    assert_eq!(interp_half.timestamp, Duration::from_secs(5));

    let interp_1 = interpolator.interpolate(1.0).unwrap();
    assert_eq!(interp_1.timestamp, Duration::from_secs(10));
}

#[tokio::test]
async fn test_drift_detector() {
    let mut detector = DriftDetector::new(Duration::from_millis(100));

    // Add samples with small drift (should not trigger correction)
    detector.add_sample(Duration::from_millis(100), Duration::from_millis(105));
    detector.add_sample(Duration::from_millis(200), Duration::from_millis(208));
    detector.add_sample(Duration::from_millis(300), Duration::from_millis(310));

    assert!(!detector.needs_correction());
    assert!(detector.get_correction().is_none());

    // Add samples with large drift (should trigger correction)
    detector.add_sample(Duration::from_millis(400), Duration::from_millis(500));
    detector.add_sample(Duration::from_millis(500), Duration::from_millis(620));

    assert!(detector.needs_correction());
    assert!(detector.get_correction().is_some());
}

#[tokio::test]
async fn test_playback_stats() {
    let controller = PlaybackController::new(Box::new(DirectSync));

    let stats = controller.get_stats().await;
    assert_eq!(stats.state, PlaybackState::Stopped);
    assert_eq!(stats.current_frame, 0);
    assert_eq!(stats.total_frames, 0);
    assert_eq!(stats.playback_time, Duration::ZERO);
    assert_eq!(stats.speed, 1.0);
}

#[tokio::test]
async fn test_step_operations() {
    let controller = PlaybackController::new(Box::new(DirectSync));

    // Create test recording with multiple frames
    let mut frames = Vec::new();
    for i in 0..5 {
        frames.push(Frame {
            frame_number: i,
            timestamp: Duration::from_secs(i as u64),
            entities: HashMap::new(),
            events: Vec::new(),
            checksum: None,
        });
    }

    let recording = Recording {
        config: RecordingConfig::default(),
        frames,
        delta_frames: Vec::new(),
        markers: Vec::new(),
        total_frames: 5,
        duration: Duration::from_secs(5),
        version: RecordingVersion::current(),
    };

    controller.load_recording(recording).await.unwrap();

    // Create a mock BRP client
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3000,
    };
    let mut brp_client = BrpClient::new(&config);

    // Test stepping forward
    controller.step_forward(&mut brp_client).await.unwrap();
    let stats = controller.get_stats().await;
    assert_eq!(stats.current_frame, 1);

    // Test stepping backward
    controller.step_backward(&mut brp_client).await.unwrap();
    let stats = controller.get_stats().await;
    assert_eq!(stats.current_frame, 0);

    // Can't step backward from frame 0
    controller.step_backward(&mut brp_client).await.unwrap();
    let stats = controller.get_stats().await;
    assert_eq!(stats.current_frame, 0);
}

#[tokio::test]
async fn test_seek_to_marker() {
    let controller = PlaybackController::new(Box::new(DirectSync));

    let recording = Recording {
        config: RecordingConfig::default(),
        frames: vec![Frame {
            frame_number: 0,
            timestamp: Duration::from_secs(0),
            entities: HashMap::new(),
            events: Vec::new(),
            checksum: None,
        }],
        delta_frames: Vec::new(),
        markers: vec![bevy_debugger_mcp::recording_system::Marker {
            name: "test_marker".to_string(),
            frame_number: 0,
            timestamp: Duration::from_secs(0),
            description: Some("Test marker".to_string()),
            metadata: HashMap::new(),
        }],
        total_frames: 1,
        duration: Duration::from_secs(1),
        version: RecordingVersion::current(),
    };

    controller.load_recording(recording).await.unwrap();

    // Seek to existing marker
    controller.seek_to_marker("test_marker").await.unwrap();
    assert_eq!(controller.get_state().await, PlaybackState::Paused);

    // Seek to non-existent marker (should fail)
    assert!(controller.seek_to_marker("nonexistent").await.is_err());
}

#[tokio::test]
async fn test_interpolated_sync_strategy() {
    let sync = InterpolatedSync::new(0.5);

    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3000,
    };
    let mut brp_client = BrpClient::new(&config);

    let frame = Frame {
        frame_number: 0,
        timestamp: Duration::from_secs(0),
        entities: HashMap::new(),
        events: Vec::new(),
        checksum: None,
    };

    // Test sync operations (currently no-ops but should not panic)
    sync.prepare(&mut brp_client).await.unwrap();
    sync.sync_frame(&frame, &mut brp_client).await.unwrap();
    sync.cleanup(&mut brp_client).await.unwrap();

    // Test drift detection
    assert!(!sync.needs_drift_correction(Duration::from_millis(100), Duration::from_millis(150)));

    assert!(sync.needs_drift_correction(Duration::from_millis(100), Duration::from_millis(250)));
}

#[tokio::test]
async fn test_playback_control_messages() {
    // Test basic functionality (PlaybackControl is private)
    let speed = PlaybackSpeed::new(2.0).unwrap();
    assert_eq!(speed.value(), 2.0);
}
