//! Diagnostics for windows the WM stops managing unexpectedly.
//!
//! Records are written to `~/.glzr/glazewm/window-diagnostics.log` (see
//! `setup_logging` in `main.rs`), so that a window found orphaned later
//! can be traced back to the code path that dropped it. Windows that are
//! simply closed are not recorded, which keeps the log small enough to
//! leave on permanently.

use crate::{models::WindowContainer, traits::WindowGetters};

/// `tracing` target routed to the window diagnostics log.
pub const TARGET: &str = "window_diagnostics";

/// Records that a window is about to be unmanaged for a reason other than
/// a destroy event.
///
/// `reason` names the code path doing the unmanaging.
pub fn record_unmanage(window: &WindowContainer, reason: &str) {
  tracing::info!(
    target: TARGET,
    "Unmanaging {window} ({reason}): {}.",
    native_state(window)
  );
}

/// Records that an orphaned window was reattached to a workspace.
///
/// # Platform-specific
///
/// Only Windows can orphan windows, so this is only available on Windows.
#[cfg(target_os = "windows")]
pub fn record_reattach(window: &WindowContainer, workspace_name: &str) {
  tracing::info!(
    target: TARGET,
    "Reattached {window} to workspace '{workspace_name}'."
  );
}

/// Describes the OS-level visibility state that decides whether the WM
/// treats a window as shown.
fn native_state(window: &WindowContainer) -> String {
  let native = window.native();

  let base = format!(
    "display_state={:?}, visible={:?}",
    window.display_state(),
    native.is_visible().ok(),
  );

  #[cfg(target_os = "windows")]
  {
    use wm_platform::NativeWindowWindowsExt;

    format!(
      "{base}, cloaked={:?}, on_current_desktop={:?}",
      native.is_cloaked().ok(),
      native.is_on_current_virtual_desktop().ok(),
    )
  }

  #[cfg(not(target_os = "windows"))]
  base
}
