//! Camera preview and controls.

pub mod controls;
pub mod preview;

/// Linux cameras need no consent prompt; nothing to request.
pub(crate) fn request_camera_access(_cx: &mut gpui::App) {}
