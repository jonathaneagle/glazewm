use anyhow::{bail, Context};

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
/// omitted. Each omission is logged at debug level, since a partly
/// restored layout is otherwise indistinguishable from a bug.
///
/// The result is a snapshot taken before any move is performed, so
/// swapping the workspaces of two monitors can transiently activate one
/// extra filler workspace on the monitor vacated first. It is
/// deactivated on the next focus change.
#[must_use]
pub fn workspaces_to_move(
  layout: &SavedLayout,
  live: &[Monitor],
) -> Vec<(Workspace, Monitor)> {
  let resolved = layout.resolve(live);
  let unmatched = layout.monitors.len() - resolved.len();

  if unmatched > 0 {
    tracing::debug!(
      "Skipping {unmatched} of {} monitor(s) in the layout: they do \
       not identify any connected display.",
      layout.monitors.len()
    );
  }

  resolved
    .into_iter()
    .flat_map(|(saved, target)| {
      let target = target.clone();

      saved.workspaces.iter().filter_map(move |name| {
        let Some(workspace) = live
          .iter()
          .flat_map(Monitor::workspaces)
          .find(|workspace| &workspace.config().name == name)
        else {
          tracing::debug!(
            "Skipping workspace '{name}' in the layout: it is not \
             active."
          );

          return None;
        };

        let Some(current) = workspace.monitor() else {
          tracing::debug!(
            "Skipping workspace '{name}' in the layout: it is not \
             attached to a monitor."
          );

          return None;
        };

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
    let workspace_name = workspace.config().name;
    let monitor_name = target.native_properties().device_name;

    move_workspace_to_monitor(&workspace, &target, state, config)
      .with_context(|| {
        format!(
          "Failed to move workspace '{workspace_name}' to monitor \
           '{monitor_name}'."
        )
      })?;
  }

  Ok(count)
}

/// Describes which layouts the user could have asked for instead.
///
/// Appended to the error when no layout can be restored, so that a
/// mistyped name or an empty store is self-diagnosing.
fn available_layouts(state: &WmState) -> String {
  if state.saved_layouts.is_readonly() {
    return "The layout store is read-only because it was saved by a \
            newer version of GlazeWM, so no layouts are available."
      .to_string();
  }

  let names = state.saved_layouts.names();

  if names.is_empty() {
    "No workspace layouts are saved.".to_string()
  } else {
    format!("Available layouts: {}.", names.join(", "))
  }
}

/// Restores a saved workspace layout, by name or by matching monitors.
///
/// When `name` is given, that layout is applied. When `name` is `None`,
/// the layout whose monitors exactly match the current display set is
/// applied. Either way, finding no layout is an error, since this command
/// is only ever invoked deliberately. Automatic restore on display change
/// does not come through here; it calls `SavedLayouts::exact_match` and
/// `apply_layout` directly, so a dock event that matches nothing stays
/// silent.
///
/// # Errors
///
/// Returns an error if no layout is found, or if applying the layout
/// fails.
pub fn restore_workspace_layout(
  name: Option<&str>,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let (restored_name, resolved) = if let Some(name) = name {
    let Some(layout) = state.saved_layouts.get(name) else {
      bail!(
        "No saved workspace layout named '{name}'. {}",
        available_layouts(state)
      );
    };

    (name.to_string(), layout.clone())
  } else {
    let Some((matched_name, layout)) =
      state.saved_layouts.exact_match(&state.monitors())
    else {
      bail!(
        "No saved workspace layout matches the connected displays. {}",
        available_layouts(state)
      );
    };

    (matched_name, layout.clone())
  };

  let count = apply_layout(&resolved, state, config)?;

  if count == 0 {
    tracing::info!(
      "Workspace layout '{restored_name}' is already in effect; no \
       workspaces moved."
    );
  } else {
    tracing::info!(
      "Restored workspace layout '{restored_name}', moving {count} \
       workspace(s)."
    );
  }

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
