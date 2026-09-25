use wm_common::WmEvent;

use crate::wm_state::WmState;

/// Re-evaluates which saved layout matches the connected displays.
///
/// Must be called whenever the display set or the layout store changes.
/// Emits `WmEvent::WorkspaceLayoutChanged` only when the active layout
/// actually changes, so callers can invoke it unconditionally.
///
/// Returns whether the active layout changed.
pub fn refresh_active_workspace_layout(state: &mut WmState) -> bool {
  let active = state.saved_layouts.active_name(&state.monitors());

  if active == state.active_workspace_layout {
    return false;
  }

  tracing::info!(
    "Active workspace layout changed to {}.",
    active
      .as_deref()
      .map_or("none".to_string(), |name| format!("'{name}'"))
  );

  state.active_workspace_layout.clone_from(&active);
  state.emit_event(WmEvent::WorkspaceLayoutChanged { name: active });

  true
}

#[cfg(test)]
mod tests {
  // Windows-only: `SavedMonitor`'s identity fields and `Monitor::mock()`'s
  // `hardware_id`/`device_path` builder params are Windows-only.
  #[cfg(target_os = "windows")]
  use tokio::sync::mpsc;
  #[cfg(target_os = "windows")]
  use wm_platform::Dispatcher;

  #[cfg(target_os = "windows")]
  use super::refresh_active_workspace_layout;
  #[cfg(target_os = "windows")]
  use crate::{
    commands::container::attach_container,
    models::Monitor,
    saved_layouts::{SavedLayout, SavedLayouts, SavedMonitor},
    wm_state::WmState,
  };

  /// Builds a state whose monitors carry the given device paths, backed by
  /// a layout store in a fresh temp directory.
  #[cfg(target_os = "windows")]
  fn state_with_monitors(paths: &[&str]) -> WmState {
    let dir = std::env::temp_dir()
      .join(format!("glazewm-refresh-{}", uuid::Uuid::new_v4()));
    let store = SavedLayouts::load(dir.join("layouts.yaml"));

    let (event_tx, _) = mpsc::unbounded_channel();
    let (exit_tx, _) = mpsc::unbounded_channel();
    let state = WmState::new(Dispatcher::mock(), event_tx, exit_tx, store);

    for path in paths {
      let monitor =
        Monitor::mock().device_path((*path).to_string()).call();

      attach_container(
        &monitor.into(),
        &state.root_container.clone().into(),
        None,
      )
      .expect("Failed to attach mock monitor.");
    }

    state
  }

  /// Builds a layout spanning monitors with the given device paths.
  #[cfg(target_os = "windows")]
  fn layout_of(paths: &[&str]) -> SavedLayout {
    SavedLayout {
      saved_at: None,
      monitors: paths
        .iter()
        .map(|path| SavedMonitor {
          hardware_id: None,
          device_path: Some((*path).to_string()),
          workspaces: vec![],
        })
        .collect(),
    }
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn picks_up_a_matching_layout_once() {
    let mut state = state_with_monitors(&["PATH-A", "PATH-B"]);
    state
      .saved_layouts
      .upsert("home", layout_of(&["PATH-A", "PATH-B"]));

    assert!(refresh_active_workspace_layout(&mut state));
    assert_eq!(state.active_workspace_layout.as_deref(), Some("home"));

    assert!(
      !refresh_active_workspace_layout(&mut state),
      "An unchanged match must not be reported as a change."
    );
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn clears_when_the_match_is_lost() {
    let mut state = state_with_monitors(&["PATH-A"]);
    state.saved_layouts.upsert("laptop", layout_of(&["PATH-A"]));
    refresh_active_workspace_layout(&mut state);

    // Re-pointing the layout at a display that is not connected is, to the
    // matcher, the same as the display being unplugged.
    state.saved_layouts.upsert("laptop", layout_of(&["PATH-Z"]));

    assert!(refresh_active_workspace_layout(&mut state));
    assert_eq!(state.active_workspace_layout, None);
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn stays_none_without_saved_layouts() {
    let mut state = state_with_monitors(&["PATH-A"]);

    assert!(!refresh_active_workspace_layout(&mut state));
    assert_eq!(state.active_workspace_layout, None);
  }
}
