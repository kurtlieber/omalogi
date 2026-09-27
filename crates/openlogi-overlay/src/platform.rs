//! Native window policy for the standalone Actions Ring overlay.

mod placement;

pub(crate) use placement::RingPlacement;

/// No native application policy is required on Linux.
pub fn configure_application() {}

/// Linux needs no additional native window configuration here.
pub fn configure_windows() {}

/// Placeholder owner for a native click-away monitor. Linux compositors expose
/// no global click monitor, so the ring keeps its in-window dismissal paths
/// (center ×, slot activation, timeout).
pub struct ClickAwayMonitor(());

/// No global click monitor exists on Linux; always `None`.
pub fn watch_clicks_outside(_on_mouse_down: impl Fn() + 'static) -> Option<ClickAwayMonitor> {
    None
}

/// One display's global geometry, in the same top-left-origin global point
/// space that `openlogi_hook::cursor_position()` reports.
pub struct CursorDisplay {
    /// Native display id, numerically equal to GPUI's `DisplayId`.
    pub id: u64,
    /// Global origin (top-left corner) of the display, in points.
    pub origin: (f64, f64),
    /// Display size in points.
    pub size: (f64, f64),
}

/// On Linux the GPUI display list already carries global origins, so there is
/// nothing to resolve natively.
pub fn display_containing(_x: f64, _y: f64) -> Option<CursorDisplay> {
    None
}
