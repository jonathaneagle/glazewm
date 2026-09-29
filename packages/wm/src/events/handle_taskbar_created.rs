use crate::wm_state::WmState;

/// Handles the event for when the native taskbar is created.
///
/// Explorer creates a new taskbar at login and whenever it restarts. The
/// new taskbar lists every window, including those the WM had removed
/// from the old one, so entries are resynced with each window's display
/// state.
pub fn handle_taskbar_created(state: &mut WmState) {
  tracing::info!("Taskbar created; queuing taskbar resync.");
  state.pending_sync.queue_taskbar_sync();
}

#[cfg(test)]
mod tests {
  use tokio::sync::mpsc;
  use wm_platform::Dispatcher;

  use super::handle_taskbar_created;
  use crate::{saved_layouts::SavedLayouts, wm_state::WmState};

  #[test]
  fn queues_taskbar_sync() {
    let dir = std::env::temp_dir()
      .join(format!("glazewm-taskbar-{}", uuid::Uuid::new_v4()));
    let store = SavedLayouts::load(dir.join("layouts.yaml"));

    let (event_tx, _) = mpsc::unbounded_channel();
    let (exit_tx, _) = mpsc::unbounded_channel();
    let mut state =
      WmState::new(Dispatcher::mock(), event_tx, exit_tx, store);

    handle_taskbar_created(&mut state);

    assert!(state.pending_sync.needs_taskbar_sync());
  }
}
