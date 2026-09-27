use anyhow::Context;

use crate::{
  commands::{
    window::{manage_window, manageable_properties},
    workspace::activate_workspace,
  },
  models::Workspace,
  user_config::UserConfig,
  window_diagnostics,
  wm_state::WmState,
};

/// Why a cloaked, `WS_VISIBLE` window is left alone rather than
/// reattached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SkipReason {
  /// The WM already manages it; it is hidden on an inactive workspace.
  Managed,

  /// It was explicitly ignored (e.g. via the `ignore` command).
  Ignored,

  /// It belongs to another virtual desktop, which is why it is cloaked.
  OtherVirtualDesktop,

  /// It is not a window the WM would manage even if it were visible.
  NotManageable,
}

/// What is known about a cloaked window when deciding whether to
/// reattach it.
#[derive(Clone, Copy, Debug)]
struct Candidate {
  managed: bool,
  ignored: bool,

  /// `None` if the virtual desktop could not be determined.
  on_current_desktop: Option<bool>,

  manageable: bool,
}

/// Decides whether a cloaked window is an orphan to reattach.
///
/// Returns `None` to reattach it, or the reason to leave it alone. A
/// window whose virtual desktop cannot be determined is left alone, since
/// reattaching it could pull it off another desktop.
fn skip_reason(candidate: Candidate) -> Option<SkipReason> {
  if candidate.managed {
    Some(SkipReason::Managed)
  } else if candidate.ignored {
    Some(SkipReason::Ignored)
  } else if candidate.on_current_desktop != Some(true) {
    Some(SkipReason::OtherVirtualDesktop)
  } else if !candidate.manageable {
    Some(SkipReason::NotManageable)
  } else {
    None
  }
}

/// Re-manages windows the WM has lost track of while they were cloaked.
///
/// The WM hides windows on inactive workspaces by cloaking them, and a
/// cloaked window is treated as invisible, so one that stops being
/// managed while cloaked is never picked up again. This finds such
/// windows on the current virtual desktop, uncloaks them, and adds them
/// to the given workspace, or the first workspace in the user config.
///
/// # Errors
///
/// Returns an error if the target workspace cannot be activated, or if
/// the cloaked windows cannot be listed.
///
/// # Platform-specific
///
/// Only Windows cloaks windows, so this is only available on Windows.
pub fn reattach_windows(
  workspace_name: Option<&str>,
  state: &mut WmState,
  config: &mut UserConfig,
) -> anyhow::Result<()> {
  use wm_platform::{DispatcherExtWindows, NativeWindowWindowsExt};

  let orphans = state
    .dispatcher
    .cloaked_windows()?
    .into_iter()
    .filter(|native_window| {
      let candidate = Candidate {
        managed: state.window_from_native(native_window).is_some(),
        ignored: state.ignored_windows.contains(native_window),
        on_current_desktop: native_window
          .is_on_current_virtual_desktop()
          .ok(),
        manageable: matches!(
          manageable_properties(native_window),
          Ok(Some(_))
        ),
      };

      match skip_reason(candidate) {
        None => true,
        Some(reason) => {
          tracing::debug!(
            "Not reattaching window {:?}: {reason:?}.",
            native_window.id()
          );
          false
        }
      }
    })
    .collect::<Vec<_>>();

  if orphans.is_empty() {
    tracing::info!("No orphaned windows to reattach.");
    return Ok(());
  }

  let workspace = target_workspace(workspace_name, state, config)?;
  let workspace_name = workspace.config().name;
  let mut count = 0;

  for native_window in orphans {
    if let Err(err) = native_window.set_cloaked(false) {
      tracing::warn!("Failed to uncloak orphaned window: {err}");
      continue;
    }

    manage_window(
      native_window.clone(),
      Some(workspace.clone().into()),
      state,
      config,
    )?;

    // Managing can still decline the window, e.g. if a window rule
    // ignores it. It is left uncloaked, like any other ignored window.
    if let Some(window) = state.window_from_native(&native_window) {
      window_diagnostics::record_reattach(&window, &workspace_name);
      count += 1;
    } else {
      tracing::info!(
        "Uncloaked orphaned window {:?}, but it was not managed.",
        native_window.id()
      );
    }
  }

  tracing::info!(
    "Reattached {count} orphaned window(s) to workspace \
     '{workspace_name}'."
  );

  Ok(())
}

/// Gets the workspace to reattach windows to, activating it if needed.
fn target_workspace(
  workspace_name: Option<&str>,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<Workspace> {
  let name = match workspace_name {
    Some(name) => name.to_string(),
    None => config
      .value
      .workspaces
      .first()
      .context("No workspaces are configured.")?
      .name
      .clone(),
  };

  if let Some(workspace) = state.workspace_by_name(&name) {
    return Ok(workspace);
  }

  activate_workspace(Some(&name), None, state, config)?;

  state
    .workspace_by_name(&name)
    .with_context(|| format!("Failed to activate workspace '{name}'."))
}

#[cfg(test)]
mod tests {
  use super::{skip_reason, Candidate, SkipReason};

  /// An orphan: unmanaged, not ignored, on this desktop and manageable.
  const ORPHAN: Candidate = Candidate {
    managed: false,
    ignored: false,
    on_current_desktop: Some(true),
    manageable: true,
  };

  #[test]
  fn reattaches_an_orphan() {
    assert_eq!(skip_reason(ORPHAN), None);
  }

  #[test]
  fn leaves_managed_windows_alone() {
    let candidate = Candidate {
      managed: true,
      ..ORPHAN
    };

    assert_eq!(skip_reason(candidate), Some(SkipReason::Managed));
  }

  #[test]
  fn leaves_ignored_windows_alone() {
    let candidate = Candidate {
      ignored: true,
      ..ORPHAN
    };

    assert_eq!(skip_reason(candidate), Some(SkipReason::Ignored));
  }

  #[test]
  fn leaves_windows_on_other_desktops_alone() {
    let candidate = Candidate {
      on_current_desktop: Some(false),
      ..ORPHAN
    };

    assert_eq!(
      skip_reason(candidate),
      Some(SkipReason::OtherVirtualDesktop)
    );
  }

  #[test]
  fn leaves_windows_with_an_unknown_desktop_alone() {
    let candidate = Candidate {
      on_current_desktop: None,
      ..ORPHAN
    };

    assert_eq!(
      skip_reason(candidate),
      Some(SkipReason::OtherVirtualDesktop),
      "An undeterminable desktop must never be pulled onto this one."
    );
  }

  #[test]
  fn leaves_unmanageable_windows_alone() {
    let candidate = Candidate {
      manageable: false,
      ..ORPHAN
    };

    assert_eq!(skip_reason(candidate), Some(SkipReason::NotManageable));
  }
}
