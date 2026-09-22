use anyhow::bail;

use crate::{
  commands::monitor::move_workspace_to_monitor,
  models::{Monitor, Workspace},
  saved_layouts::SavedLayout,
  traits::CommonGetters,
  user_config::UserConfig,
  wm_state::WmState,
};

/// Determines which workspaces need moving to satisfy a layout.
///
/// Returns each workspace paired with the monitor it should move to.
/// Workspaces already on their target, workspaces the layout does not
/// name, and entries for monitors that are not connected are all
/// omitted.
#[must_use]
pub fn workspaces_to_move(
  layout: &SavedLayout,
  live: &[Monitor],
) -> Vec<(Workspace, Monitor)> {
  layout
    .resolve(live)
    .into_iter()
    .flat_map(|(saved, target)| {
      let target = target.clone();

      saved.workspaces.iter().filter_map(move |name| {
        let workspace = live
          .iter()
          .flat_map(Monitor::workspaces)
          .find(|workspace| &workspace.config().name == name)?;

        let current = workspace.monitor()?;

        // Skip workspaces that are already where they belong.
        (current.id() != target.id()).then(|| (workspace, target.clone()))
      })
    })
    .collect()
}

/// Applies a saved layout to the current monitors.
///
/// Returns the number of workspaces moved.
///
/// # Errors
///
/// Returns an error if a workspace cannot be moved.
pub fn apply_layout(
  layout: &SavedLayout,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<usize> {
  let moves = workspaces_to_move(layout, &state.monitors());
  let count = moves.len();

  for (workspace, target) in moves {
    move_workspace_to_monitor(&workspace, &target, state, config)?;
  }

  Ok(count)
}

/// Restores a saved workspace layout, by name or by matching monitors.
///
/// When `name` is given, that layout is applied, erroring if no layout
/// with that name is saved. When `name` is `None`, the layout whose
/// monitors exactly match the current display set is applied, if any.
///
/// # Errors
///
/// Returns an error if a named layout is not found, or if applying the
/// layout fails.
pub fn restore_workspace_layout(
  name: Option<&str>,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let resolved = if let Some(name) = name {
    let Some(layout) = state.saved_layouts.get(name) else {
      if state.saved_layouts.is_readonly() {
        bail!(
          "Layout store is read-only because it was saved by a newer \
           version of GlazeWM. No layouts are available."
        );
      }

      let available = state.saved_layouts.names().join(", ");

      bail!(
        "No saved workspace layout named '{name}'. Available \
         layouts: {available}."
      );
    };

    layout.clone()
  } else {
    let Some((_, layout)) =
      state.saved_layouts.exact_match(&state.monitors())
    else {
      return Ok(());
    };

    layout.clone()
  };

  let count = apply_layout(&resolved, state, config)?;

  tracing::info!(
    "Restored workspace layout, moving {count} workspace(s)."
  );

  Ok(())
}

#[cfg(test)]
mod tests {
  // Windows-only: `SavedMonitor`'s `hardware_id` field and
  // `Monitor::mock()`'s `hardware_id` builder param are Windows-only.
  #[cfg(target_os = "windows")]
  use super::workspaces_to_move;
  #[cfg(target_os = "windows")]
  use crate::{
    models::{Monitor, Workspace},
    saved_layouts::{SavedLayout, SavedMonitor},
  };

  /// Builds a saved monitor entry.
  #[cfg(target_os = "windows")]
  fn saved(hardware_id: &str, workspaces: &[&str]) -> SavedMonitor {
    SavedMonitor {
      hardware_id: Some(hardware_id.to_string()),
      device_path: None,
      workspaces: workspaces.iter().map(|n| (*n).to_string()).collect(),
    }
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn moves_only_workspaces_that_are_elsewhere() {
    // Workspace 1 is already on the left monitor; workspace 3 is not.
    let left = Monitor::mock()
      .hardware_id("LEFT".to_string())
      .workspaces(vec![Workspace::mock().name("1".to_string()).call()])
      .call();

    let right = Monitor::mock()
      .hardware_id("RIGHT".to_string())
      .workspaces(vec![Workspace::mock().name("3".to_string()).call()])
      .call();

    let live = vec![left, right];

    let layout = SavedLayout {
      saved_at: None,
      monitors: vec![saved("LEFT", &["1", "3"]), saved("RIGHT", &[])],
    };

    let moves = workspaces_to_move(&layout, &live);

    assert_eq!(moves.len(), 1, "Only workspace 3 should move.");
    assert_eq!(moves[0].0.config().name, "3");
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn ignores_workspaces_not_in_the_layout() {
    let left = Monitor::mock()
      .hardware_id("LEFT".to_string())
      .workspaces(vec![Workspace::mock().name("9".to_string()).call()])
      .call();

    let live = vec![left];

    let layout = SavedLayout {
      saved_at: None,
      monitors: vec![saved("LEFT", &[])],
    };

    assert!(workspaces_to_move(&layout, &live).is_empty());
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn skips_unmatched_monitors() {
    let left = Monitor::mock()
      .hardware_id("LEFT".to_string())
      .workspaces(vec![Workspace::mock().name("1".to_string()).call()])
      .call();

    let live = vec![left];

    // The layout references a monitor that is not connected.
    let layout = SavedLayout {
      saved_at: None,
      monitors: vec![saved("ABSENT", &["1"])],
    };

    assert!(workspaces_to_move(&layout, &live).is_empty());
  }
}
