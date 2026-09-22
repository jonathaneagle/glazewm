// Temporary: `capture_layout` is only consumed by tests until Task 8
// wires it into `save_workspace_layout`, when this allow is removed.
#![allow(dead_code)]

use crate::{
  models::Monitor,
  saved_layouts::{SavedLayout, SavedMonitor},
};

/// Builds a layout describing which workspaces sit on which monitors.
#[must_use]
pub fn capture_layout(monitors: &[Monitor]) -> SavedLayout {
  let saved_monitors = monitors
    .iter()
    .map(|monitor| {
      let properties = monitor.native_properties();

      let workspaces = monitor
        .workspaces()
        .iter()
        .map(|workspace| workspace.config().name)
        .collect();

      SavedMonitor {
        #[cfg(target_os = "windows")]
        hardware_id: properties.hardware_id.clone(),
        #[cfg(target_os = "windows")]
        device_path: properties.device_path.clone(),
        #[cfg(target_os = "macos")]
        device_uuid: Some(properties.device_uuid.clone()),
        workspaces,
      }
    })
    .collect();

  SavedLayout {
    saved_at: None,
    monitors: saved_monitors,
  }
}

#[cfg(test)]
mod tests {
  #[cfg(target_os = "windows")]
  use super::capture_layout;
  #[cfg(target_os = "windows")]
  use crate::models::{Monitor, Workspace};

  #[cfg(target_os = "windows")]
  #[test]
  fn captures_workspaces_per_monitor() {
    let left = Monitor::mock()
      .hardware_id("DELA26B".to_string())
      .device_path("PATH-LEFT".to_string())
      .workspaces(vec![
        Workspace::mock().name("1".to_string()).call(),
        Workspace::mock().name("5".to_string()).call(),
      ])
      .call();

    let right = Monitor::mock()
      .hardware_id("AUO82B2".to_string())
      .device_path("PATH-RIGHT".to_string())
      .workspaces(vec![Workspace::mock().name("3".to_string()).call()])
      .call();

    let layout = capture_layout(&[left, right]);

    assert_eq!(layout.monitors.len(), 2);
    assert_eq!(layout.monitors[0].hardware_id.as_deref(), Some("DELA26B"));
    assert_eq!(
      layout.monitors[0].workspaces,
      vec!["1".to_string(), "5".to_string()]
    );
    assert_eq!(layout.monitors[1].workspaces, vec!["3".to_string()]);
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn captures_monitor_with_no_workspaces() {
    let monitor = Monitor::mock().hardware_id("EMPTY".to_string()).call();

    let layout = capture_layout(&[monitor]);

    assert_eq!(layout.monitors.len(), 1);
    assert_eq!(layout.monitors[0].workspaces, Vec::<String>::new());
  }
}
