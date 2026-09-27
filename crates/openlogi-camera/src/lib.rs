//! Generic discovery of Logitech USB Video Class (UVC) webcams.
//!
//! Mice and keyboards speak Logitech's proprietary HID++ (over a Bolt/Unifying
//! receiver or directly) — see the `openlogi-hid` crate. Webcams don't: every
//! Logitech camera (StreamCam, Brio, C920, C922, C270, C930e, …) is a standard
//! UVC device and enumerates the same way. So detection keys off the USB vendor
//! id (`0x046d`) rather than any per-model quirk — plug in *any* Logitech
//! camera and it's recognised, with no model table to maintain.
//!
//! Linux uses V4L2 for both capture and UVC controls, through the kernel's
//! `uvcvideo` driver. Other platforms return an empty list.

use serde::Serialize;

mod controls;
pub use controls::{AutoState, AutoToggle, CameraControl, CameraState, ControlError, ControlRange};

mod capture_types;
pub use capture_types::{CaptureError, Frame};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
mod capture_linux;
#[cfg(target_os = "linux")]
pub use capture_linux::{
    CameraStream, camera_access_granted, camera_authorization, capture_frame,
    request_camera_access, start_stream,
};

#[cfg(target_os = "linux")]
mod uvc_linux;
#[cfg(target_os = "linux")]
pub use uvc_linux::{
    apply_settings, control_range, control_ranges, read_camera_state, set_auto, set_control,
};

#[cfg(not(target_os = "linux"))]
mod capture {
    //! Stub capture backend for platforms without one.
    use std::sync::Arc;
    use std::time::Duration;

    use crate::capture_types::{CaptureError, Frame};

    /// Stub: returns [`CaptureError::Unsupported`] on this platform.
    pub fn capture_frame(_unique_id: &str, _timeout: Duration) -> Result<Frame, CaptureError> {
        Err(CaptureError::Unsupported)
    }

    /// Stub live stream (never yields a frame on this platform).
    pub struct CameraStream;

    impl CameraStream {
        #[must_use]
        pub fn latest_frame(&self) -> Option<Arc<Frame>> {
            None
        }

        #[must_use]
        pub fn take_frame(&self) -> Option<Arc<Frame>> {
            None
        }

        #[must_use]
        pub fn frame_generation(&self) -> u64 {
            0
        }
    }

    /// Stub: returns [`CaptureError::Unsupported`] on this platform.
    pub fn start_stream(_unique_id: &str) -> Result<CameraStream, CaptureError> {
        Err(CaptureError::Unsupported)
    }

    /// Stub: camera access is never granted on this platform.
    #[must_use]
    pub fn camera_access_granted() -> bool {
        false
    }

    /// Stub: camera permission is always undetermined on this platform.
    #[must_use]
    pub fn camera_authorization() -> crate::CameraAuthorization {
        crate::CameraAuthorization::Undetermined
    }

    /// Stub: no consent prompt exists on this platform.
    pub fn request_camera_access() {}
}
#[cfg(not(target_os = "linux"))]
pub use capture::{
    CameraStream, camera_access_granted, camera_authorization, capture_frame,
    request_camera_access, start_stream,
};

#[cfg(not(target_os = "linux"))]
mod uvc {
    //! Stub UVC control backend for platforms without one.
    use crate::controls::{AutoToggle, CameraControl, CameraState, ControlError, ControlRange};

    /// Stub: no UVC backend on this platform.
    pub fn control_range(_id: &str, _c: CameraControl) -> Result<ControlRange, ControlError> {
        Err(ControlError::Unsupported)
    }

    /// Stub: no UVC backend on this platform.
    pub fn control_ranges(_id: &str) -> Result<Vec<(CameraControl, ControlRange)>, ControlError> {
        Ok(Vec::new())
    }

    /// Stub: no UVC backend on this platform.
    pub fn read_camera_state(_id: &str) -> Result<CameraState, ControlError> {
        Ok(CameraState::default())
    }

    /// Stub: no UVC backend on this platform.
    pub fn set_control(_id: &str, _c: CameraControl, _value: i32) -> Result<(), ControlError> {
        Err(ControlError::Unsupported)
    }

    /// Stub: no UVC backend on this platform.
    pub fn set_auto(_id: &str, _t: AutoToggle, _on: bool) -> Result<(), ControlError> {
        Err(ControlError::Unsupported)
    }

    /// Stub: no UVC backend on this platform.
    pub fn apply_settings(
        _id: &str,
        _autos: &[(AutoToggle, bool)],
        _values: &[(CameraControl, i32)],
    ) -> Result<(), ControlError> {
        Err(ControlError::Unsupported)
    }
}
#[cfg(not(target_os = "linux"))]
pub use uvc::{
    apply_settings, control_range, control_ranges, read_camera_state, set_auto, set_control,
};

/// Logitech's USB vendor id. Reported in decimal (`1133`) inside an
/// `AVCaptureDevice` modelID, and in hex (`046d`) most everywhere else.
pub use openlogi_device_registry::LOGITECH_VENDOR_ID as LOGITECH_VID;

/// Tri-state Camera permission, mirroring macOS `AVAuthorizationStatus`.
///
/// Only macOS has a consent model with a pending state. Linux decides access
/// by filesystem permission on the device node, so it reports `Granted` or
/// `Denied` but never `Undetermined`; platforms with no backend at all report
/// `Undetermined`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraAuthorization {
    /// The process may open cameras.
    Granted,
    /// The user denied access, or the system restricts it.
    Denied,
    /// Not yet requested — opening a camera will prompt.
    Undetermined,
}

/// A connected USB Video Class camera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Camera {
    /// Human-readable name, e.g. `"Logitech StreamCam"`.
    pub name: String,
    /// OS capture-layer identifier (AVFoundation `uniqueID`, DirectShow device
    /// path). Used to open preview/controls; may embed a USB location and so
    /// change when the camera is moved to another port.
    pub unique_id: String,
    /// USB `iSerialNumber` when the device reports one. Port-stable; preferred
    /// for persisted config keys via [`Self::config_key`].
    pub serial_number: Option<String>,
    /// USB vendor id (`0x046d` for Logitech).
    pub vendor_id: u16,
    /// USB product id (e.g. `0x0893` for the StreamCam).
    pub product_id: u16,
    /// Largest supported frame size `(width, height)`, when the OS reports the
    /// device's formats. Read from metadata only — no capture, no permission.
    pub max_resolution: Option<(u32, u32)>,
    /// Highest supported frame rate (fps) across all formats, when known.
    pub max_fps: Option<u32>,
}

impl Camera {
    /// Persistence key that is stable across USB ports.
    ///
    /// Prefers the USB serial when the device reports one. When it doesn't,
    /// falls back to a model-scoped key (`camera:vid:pid`) so settings survive
    /// a port change. Two serial-less units of the same model share this key
    /// (no stronger USB identity); the GUI keeps them as separate live cards
    /// via the OS capture id, not via this settings key.
    #[must_use]
    pub fn config_key(&self) -> String {
        if let Some(serial) = self
            .serial_number
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            format!(
                "camera:{:04x}:{:04x}:serial:{}",
                self.vendor_id,
                self.product_id,
                serial.to_ascii_lowercase()
            )
        } else {
            format!("camera:{:04x}:{:04x}", self.vendor_id, self.product_id)
        }
    }
}

/// Whether this platform has a live-capture backend (preview + snapshot).
/// Enumeration and UVC controls can be supported without it.
#[must_use]
pub const fn capture_supported() -> bool {
    cfg!(target_os = "linux")
}

/// Enumerate every connected **Logitech** UVC camera.
///
/// Non-Logitech cameras (the built-in FaceTime camera, virtual cameras, other
/// vendors' webcams) are filtered out. Returns an empty list on platforms with
/// no capture backend, or when no Logitech camera is attached.
#[must_use]
pub fn enumerate_cameras() -> Vec<Camera> {
    enumerate_all()
        .into_iter()
        .filter(|camera| camera.vendor_id == LOGITECH_VID)
        .collect()
}

#[cfg(target_os = "linux")]
fn enumerate_all() -> Vec<Camera> {
    linux::nodes().iter().map(linux::describe).collect()
}

#[cfg(not(target_os = "linux"))]
fn enumerate_all() -> Vec<Camera> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_key_prefers_usb_serial_over_capture_id() {
        let with_serial = Camera {
            name: "Logitech StreamCam".into(),
            unique_id: "0x1123000046d0893".into(),
            serial_number: Some("ABC123".into()),
            vendor_id: LOGITECH_VID,
            product_id: 0x0893,
            max_resolution: None,
            max_fps: None,
        };
        assert_eq!(with_serial.config_key(), "camera:046d:0893:serial:abc123");
        // Same physical camera on another USB port → same config key.
        let moved = Camera {
            unique_id: "0x14110000046d0893".into(),
            ..with_serial.clone()
        };
        assert_eq!(moved.config_key(), with_serial.config_key());

        let no_serial = Camera {
            serial_number: None,
            unique_id: "0x1123000046d0893".into(),
            ..with_serial.clone()
        };
        // Model-scoped — same key after a port change even without a serial.
        assert_eq!(no_serial.config_key(), "camera:046d:0893");
        let moved_no_serial = Camera {
            unique_id: "0x14110000046d0893".into(),
            ..no_serial
        };
        assert_eq!(moved_no_serial.config_key(), "camera:046d:0893");
    }
}
