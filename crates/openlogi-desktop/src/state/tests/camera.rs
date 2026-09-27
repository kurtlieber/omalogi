//! Camera settings: the legacy-key migration and camera-triggered automation.

use super::*;

fn camera_controls(brightness: i32) -> openlogi_core::config::CameraControls {
    openlogi_core::config::CameraControls(std::collections::BTreeMap::from([(
        "brightness".into(),
        brightness,
    )]))
}

fn camera_state(config: Config) -> AppState {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    AppState::new(Sources::in_memory(config, &AssetResolver::new(), tx))
}

#[test]
fn migrate_lifts_legacy_port_bound_camera_key() {
    let mut config = Config::ephemeral();
    let model = "camera:046d:0893";
    let legacy = "camera-0x1123000046d0893";
    config.set_camera_controls(legacy, camera_controls(42));
    let mut state = camera_state(config);

    state.migrate_legacy_camera_key(model, "0x1123000046d0893");

    assert_eq!(
        state
            .config
            .camera_controls(model)
            .map(|c| c.0["brightness"]),
        Some(42)
    );
    assert!(state.config.camera_controls(legacy).is_none());
}

#[test]
fn migrate_does_not_overwrite_existing_model_settings() {
    let mut config = Config::ephemeral();
    let model = "camera:046d:0893";
    let legacy = "camera-0x1123000046d0893";
    config.set_camera_controls(model, camera_controls(1));
    config.set_camera_controls(legacy, camera_controls(99));
    let mut state = camera_state(config);

    state.migrate_legacy_camera_key(model, "0x1123000046d0893");

    assert_eq!(
        state
            .config
            .camera_controls(model)
            .map(|c| c.0["brightness"]),
        Some(1)
    );
    assert_eq!(
        state
            .config
            .camera_controls(legacy)
            .map(|c| c.0["brightness"]),
        Some(99)
    );
}
