use anyhow::bail;

use crate::{
  commands::{
    monitor::move_workspace_to_monitor, workspace::activate_workspace,
  },
  models::{Monitor, Workspace},
  traits::CommonGetters,
  user_config::UserConfig,
  wm_state::WmState,
};

/// Gives a monitor a workspace if it has none.
///
/// Activates an inactive workspace from the user config when one is
/// available. When every configured workspace is already active, moves a
/// hidden workspace from another monitor instead (see
/// `workspace_to_borrow`), since a monitor without a workspace is an
/// invalid state that other commands cannot recover from.
///
/// # Errors
///
/// Returns an error if the monitor has no workspace and none can be
/// activated or borrowed, which means there are more monitors than
/// configured workspaces.
pub fn ensure_monitor_has_workspace(
  monitor: &Monitor,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  if monitor.child_count() > 0 {
    return Ok(());
  }

  // `activate_workspace` succeeds whenever any configured workspace is
  // inactive, preferring one bound to this monitor.
  if config
    .next_inactive_workspace_config(&state.workspaces())
    .is_some()
  {
    return activate_workspace(None, Some(monitor.clone()), state, config);
  }

  let Some(workspace) = workspace_to_borrow(&state.monitors(), monitor)
  else {
    bail!(
      "Monitor {monitor} has no workspace, and none can be spared. \
       Configure at least as many workspaces as there are monitors."
    );
  };

  tracing::info!(
    "Every configured workspace is active, so moving workspace '{}' to \
     monitor {monitor}, which had none.",
    workspace.config().name
  );

  move_workspace_to_monitor(&workspace, monitor, state, config)
}

/// Chooses a workspace to move onto an empty monitor when no inactive
/// workspace is left to activate.
///
/// Takes from the monitor with the most workspaces, and never takes a
/// monitor's displayed workspace or its only one, so no monitor visibly
/// changes or is left empty. Of that monitor's hidden workspaces, the one
/// last in config order is chosen, since workspaces are sorted by config
/// order.
///
/// Returns `None` if no other monitor has a workspace to spare.
fn workspace_to_borrow(
  monitors: &[Monitor],
  target: &Monitor,
) -> Option<Workspace> {
  monitors
    .iter()
    .filter(|monitor| monitor.id() != target.id())
    .filter(|monitor| monitor.child_count() > 1)
    // `max_by_key` keeps the last of equal maxima, so iterate in reverse
    // to prefer the first monitor on a tie.
    .rev()
    .max_by_key(|monitor| monitor.child_count())
    .and_then(|monitor| {
      let displayed = monitor.displayed_workspace();

      monitor.workspaces().into_iter().rev().find(|workspace| {
        displayed
          .as_ref()
          .is_none_or(|displayed| displayed.id() != workspace.id())
      })
    })
}

#[cfg(test)]
mod tests {
  use super::workspace_to_borrow;
  use crate::{
    commands::container::set_focused_descendant,
    models::{Monitor, Workspace},
  };

  /// Builds a monitor holding workspaces with the given names, displaying
  /// the one named `displayed`.
  fn monitor_with(names: &[&str], displayed: Option<&str>) -> Monitor {
    let monitor = Monitor::mock()
      .workspaces(
        names
          .iter()
          .map(|name| Workspace::mock().name((*name).to_string()).call())
          .collect(),
      )
      .call();

    if let Some(workspace) = displayed.and_then(|displayed| {
      monitor
        .workspaces()
        .into_iter()
        .find(|workspace| workspace.config().name == displayed)
    }) {
      set_focused_descendant(&workspace.into(), None);
    }

    monitor
  }

  /// Name of the workspace that would be borrowed for `target`.
  fn borrowed(monitors: &[Monitor], target: &Monitor) -> Option<String> {
    workspace_to_borrow(monitors, target)
      .map(|workspace| workspace.config().name)
  }

  #[test]
  fn test_setup_controls_the_displayed_workspace() {
    let monitor = monitor_with(&["1", "2", "3"], Some("2"));

    assert_eq!(
      monitor
        .displayed_workspace()
        .map(|workspace| workspace.config().name)
        .as_deref(),
      Some("2"),
      "The other tests rely on choosing the displayed workspace."
    );
  }

  #[test]
  fn borrows_from_the_busiest_monitor() {
    let quiet = monitor_with(&["4", "5"], Some("4"));
    let busy = monitor_with(&["1", "2", "3"], Some("1"));
    let empty = monitor_with(&[], None);

    assert_eq!(
      borrowed(&[quiet, busy, empty.clone()], &empty).as_deref(),
      Some("3")
    );
  }

  #[test]
  fn never_takes_the_displayed_workspace() {
    let busy = monitor_with(&["1", "2", "3"], Some("3"));
    let empty = monitor_with(&[], None);

    assert_eq!(
      borrowed(&[busy, empty.clone()], &empty).as_deref(),
      Some("2"),
      "The last workspace is displayed, so the next one back is taken."
    );
  }

  #[test]
  fn never_empties_a_monitor_with_one_workspace() {
    let single = monitor_with(&["1"], Some("1"));
    let empty = monitor_with(&[], None);

    assert_eq!(borrowed(&[single, empty.clone()], &empty), None);
  }

  #[test]
  fn prefers_the_first_monitor_on_a_tie() {
    let first = monitor_with(&["1", "2"], Some("1"));
    let second = monitor_with(&["3", "4"], Some("3"));
    let empty = monitor_with(&[], None);

    assert_eq!(
      borrowed(&[first, second, empty.clone()], &empty).as_deref(),
      Some("2")
    );
  }

  #[test]
  fn never_borrows_from_the_target_itself() {
    let target = monitor_with(&["1", "2", "3"], Some("1"));

    assert_eq!(borrowed(std::slice::from_ref(&target), &target), None);
  }
}
