# Workspace Banks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a second bank of workspaces (`B1`–`B9`) on the same number keys. The active bank always follows the focused workspace, and `alt+.` jumps to the most recent workspace in the other bank.

**Architecture:** Three generic WM features, with banks themselves done purely in user config:

1. Binding modes get an `inherit` flag, so unbound keys fall through to the layer below.
2. Binding modes become a stack that keeps inheriting modes underneath a newly enabled mode.
3. Workspaces can name a linked binding mode. A sync step after every event or command keeps that mode on the stack while the workspace is focused, and records the workspace in a most-recent-first focus history used by two new `focus` flags.

**Tech Stack:** Rust (nightly), `serde`/`serde_yaml`, `clap`, `bon` mocks, Zebar widget (plain JS).

**Spec:** `docs/superpowers/specs/2026-10-01-workspace-banks-design.md`

## Global Constraints

- Existing configs with no `inherit` and no workspace `binding_mode` must behave exactly as today.
- Error handling: `anyhow` in `wm`/`wm-common`. Avoid `.unwrap()`; `.expect("…")` is acceptable only in tests.
- Every function documented with `///`, comments end with punctuation, type names in backticks.
- Formatting: `rustfmt.toml` (2-space indent, 75-col max). Clippy pedantic warnings fail CI.
- Gates: run `cargo fmt` (code blocks here are not pre-formatted), then `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` must pass at the end of every task.
- Commits: semantic prefix (`feat:`, `test:`, `docs:`). End each message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_016cvcam1fPfdvhGDyedMvRm
  ```
- Never run `cargo` in the background (it deadlocks on the build-dir lock).
- Branch `feat/workspace-banks`, based on the long-lived `fork` integration branch (which already contains the workspace layout profiles work). When the plan is complete, merge into `fork`. Do not push; the user pushes manually.
- Bank B workspace names are exactly `B1`–`B9`. The `.` key is `oem_period` (`wm-platform/src/models/key.rs:415`).

## Review Focus

1. **Config reload while on a B workspace.** The stack is cleared, and the user expects to stay in bank B. The sync must re-add `bank-b` on the next pass (Task 4, `reload_resync_restores_linked_mode`).
2. **Focus moved by the OS (notification, taskbar click) onto a B workspace.** The user expects bank B keys. `process_event` must sync, not only `process_commands` (Task 4 Step 6 wiring, Task 8 live check 6).
3. **WM started while focus is on a B workspace.** The user expects bank B immediately, not after the first event. `populate` must sync once initialised (Task 4 Step 6, Task 8 live check 9).
4. **`alt+.` before ever visiting the other bank.** The user expects to land on `B1` (or `1`), not get an error (Task 5, `recent_falls_back_to_config_order`).
5. **A non-inheriting mode (e.g. `resize` via `alt+r`) enabled while in bank B.** The user expects to return to bank B on exit, not bank A (Task 2 `enabling_replaces_only_non_inheriting_top`, Task 8 live check 4).

---

### Task 1: `inherit` flag and layered keybinding resolution

**Files:**
- Modify: `packages/wm-common/src/parsed_config.rs` (`BindingModeConfig`)
- Modify: `packages/wm/src/user_config.rs` (`active_keybinding_configs`, new `resolve_keybinding_layers`, `#[cfg(test)] UserConfig::mock`, tests)
- Modify: `resources/assets/sample-config.yaml` (document `inherit`)

**Interfaces:**
- Produces: `BindingModeConfig.inherit: bool`.
- Produces: `fn resolve_keybinding_layers(binding_modes: &[BindingModeConfig], base_keybindings: &[KeybindingConfig]) -> Vec<KeybindingConfig>` (private to `user_config.rs`).
- Produces: `#[cfg(test)] pub fn UserConfig::mock(config_str: &str) -> UserConfig`.
- Unchanged signature: `UserConfig::active_keybinding_configs(&self, binding_modes: &[BindingModeConfig], is_paused: bool) -> impl Iterator<Item = KeybindingConfig>`. The top of the stack is now the **last** element.

- [ ] **Step 1: Add the field**

In `packages/wm-common/src/parsed_config.rs`, inside `BindingModeConfig`, after `display_name`:

```rs
  /// Whether keys this mode does not bind fall through to the layer
  /// below (the next active binding mode, or the base `keybindings`).
  #[serde(default)]
  pub inherit: bool,
```

- [ ] **Step 2: Add `UserConfig::mock` and the failing tests**

At the bottom of `packages/wm/src/user_config.rs`:

```rs
#[cfg(test)]
impl UserConfig {
  /// Creates a `UserConfig` from a YAML string for use in tests.
  ///
  /// # Panics
  ///
  /// Panics if the YAML is not a valid config.
  pub fn mock(config_str: &str) -> Self {
    let value: ParsedConfig =
      serde_yaml::from_str(config_str).expect("Invalid mock config.");

    Self {
      path: PathBuf::new(),
      window_rules_by_event: Self::window_rules_by_event(&value),
      value,
      value_str: config_str.to_string(),
    }
  }
}

#[cfg(test)]
mod tests {
  use wm_common::{BindingModeConfig, InvokeCommand, KeybindingConfig};
  use wm_platform::Keybinding;

  use super::{resolve_keybinding_layers, UserConfig};

  const CONFIG: &str = r"
keybindings:
  - commands: ['focus --workspace 1']
    bindings: ['alt+1']
  - commands: ['focus --direction left']
    bindings: ['alt+h', 'alt+left']
  - commands: ['wm-toggle-pause']
    bindings: ['alt+shift+p']

binding_modes:
  - name: 'bank-b'
    inherit: true
    keybindings:
      - commands: ['focus --workspace B1']
        bindings: ['alt+1']
      - commands: ['focus --direction right']
        bindings: ['alt+h']
  - name: 'legend'
    keybindings:
      - commands: ['focus --workspace 1']
        bindings: ['1']
";

  /// Parses a keybinding string (e.g. `alt+1`) with the config parser.
  fn keybinding(value: &str) -> Keybinding {
    let config: KeybindingConfig = serde_yaml::from_str(&format!(
      "{{ bindings: ['{value}'], commands: [] }}"
    ))
    .expect("Invalid keybinding.");

    config.bindings[0].clone()
  }

  /// Parses a single command string (e.g. `focus --workspace 1`).
  fn command(value: &str) -> InvokeCommand {
    serde_yaml::from_str(&format!("'{value}'")).expect("Invalid command.")
  }

  /// Gets the commands that a key resolves to, if any.
  fn commands_for(
    resolved: &[KeybindingConfig],
    key: &str,
  ) -> Option<Vec<InvokeCommand>> {
    let key = keybinding(key);

    resolved
      .iter()
      .find(|config| config.bindings.contains(&key))
      .map(|config| config.commands.clone())
  }

  /// Gets the configured binding modes with the given names, in order.
  fn modes(config: &UserConfig, names: &[&str]) -> Vec<BindingModeConfig> {
    names
      .iter()
      .map(|name| {
        config
          .value
          .binding_modes
          .iter()
          .find(|mode| mode.name == *name)
          .cloned()
          .expect("Unknown binding mode.")
      })
      .collect()
  }

  #[test]
  fn empty_stack_resolves_to_base_keybindings() {
    let config = UserConfig::mock(CONFIG);
    let resolved =
      resolve_keybinding_layers(&[], &config.value.keybindings);

    assert_eq!(
      commands_for(&resolved, "alt+1"),
      Some(vec![command("focus --workspace 1")])
    );
    assert_eq!(commands_for(&resolved, "1"), None);
  }

  #[test]
  fn non_inheriting_mode_hides_base_keybindings() {
    let config = UserConfig::mock(CONFIG);
    let resolved = resolve_keybinding_layers(
      &modes(&config, &["legend"]),
      &config.value.keybindings,
    );

    assert_eq!(
      commands_for(&resolved, "1"),
      Some(vec![command("focus --workspace 1")])
    );
    assert_eq!(commands_for(&resolved, "alt+1"), None);
  }

  #[test]
  fn inheriting_mode_overrides_and_falls_through() {
    let config = UserConfig::mock(CONFIG);
    let resolved = resolve_keybinding_layers(
      &modes(&config, &["bank-b"]),
      &config.value.keybindings,
    );

    assert_eq!(
      commands_for(&resolved, "alt+1"),
      Some(vec![command("focus --workspace B1")])
    );
    assert_eq!(
      commands_for(&resolved, "alt+shift+p"),
      Some(vec![command("wm-toggle-pause")])
    );
  }

  #[test]
  fn partially_shadowed_keybinding_keeps_unshadowed_keys() {
    let config = UserConfig::mock(CONFIG);
    let resolved = resolve_keybinding_layers(
      &modes(&config, &["bank-b"]),
      &config.value.keybindings,
    );

    assert_eq!(
      commands_for(&resolved, "alt+h"),
      Some(vec![command("focus --direction right")])
    );
    assert_eq!(
      commands_for(&resolved, "alt+left"),
      Some(vec![command("focus --direction left")])
    );
  }

  #[test]
  fn shadowed_keys_are_registered_once() {
    let config = UserConfig::mock(CONFIG);
    let resolved = resolve_keybinding_layers(
      &modes(&config, &["bank-b"]),
      &config.value.keybindings,
    );
    let key = keybinding("alt+1");

    let count = resolved
      .iter()
      .flat_map(|config| config.bindings.iter())
      .filter(|binding| **binding == key)
      .count();

    assert_eq!(count, 1);
  }

  #[test]
  fn non_inheriting_top_stops_resolution() {
    let config = UserConfig::mock(CONFIG);
    let resolved = resolve_keybinding_layers(
      &modes(&config, &["bank-b", "legend"]),
      &config.value.keybindings,
    );

    assert_eq!(
      commands_for(&resolved, "1"),
      Some(vec![command("focus --workspace 1")])
    );
    assert_eq!(commands_for(&resolved, "alt+1"), None);
  }

  #[test]
  fn paused_keeps_only_toggle_pause() {
    let config = UserConfig::mock(CONFIG);
    let resolved = config
      .active_keybinding_configs(&modes(&config, &["bank-b"]), true)
      .collect::<Vec<_>>();

    assert_eq!(resolved.len(), 1);
    assert_eq!(
      commands_for(&resolved, "alt+shift+p"),
      Some(vec![command("wm-toggle-pause")])
    );
  }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p wm user_config`
Expected: compile error `cannot find function resolve_keybinding_layers`.

- [ ] **Step 4: Implement the resolution**

In `packages/wm/src/user_config.rs`, replace the whole `active_keybinding_configs` method with:

```rs
  /// Keybinding configs that should be active for the current binding
  /// mode stack and pause state.
  ///
  /// See `resolve_keybinding_layers` for how the stack is flattened.
  ///
  /// When paused, only the configs with `InvokeCommand::WmTogglePause` are
  /// returned so that unpausing remains possible.
  pub fn active_keybinding_configs(
    &self,
    binding_modes: &[wm_common::BindingModeConfig],
    is_paused: bool,
  ) -> impl Iterator<Item = KeybindingConfig> {
    resolve_keybinding_layers(binding_modes, &self.value.keybindings)
      .into_iter()
      .filter(move |kb| {
        !is_paused
          || kb
            .commands
            .contains(&wm_common::InvokeCommand::WmTogglePause)
      })
  }
```

Then add this free function directly after the `impl UserConfig { … }` block, before the `#[cfg(test)]` items:

```rs
/// Flattens a binding mode stack and the base keybindings into the
/// keybinding configs that are currently reachable, highest priority
/// first.
///
/// Layers are taken from the top of the stack (last element) downwards,
/// continuing past a mode only if it has `inherit: true`, and ending with
/// `base_keybindings` if every mode inherits. A key bound by a higher
/// layer is removed from lower layers, so each key resolves to exactly
/// one config and is registered with the keyboard hook once.
fn resolve_keybinding_layers(
  binding_modes: &[wm_common::BindingModeConfig],
  base_keybindings: &[KeybindingConfig],
) -> Vec<KeybindingConfig> {
  let mut layers = Vec::new();
  let mut reaches_base = true;

  for mode in binding_modes.iter().rev() {
    layers.push(mode.keybindings.as_slice());

    if !mode.inherit {
      reaches_base = false;
      break;
    }
  }

  if reaches_base {
    layers.push(base_keybindings);
  }

  let mut bound_keys = Vec::new();
  let mut resolved = Vec::new();

  for layer in layers {
    let mut layer_keys = Vec::new();

    for keybinding_config in layer {
      let bindings = keybinding_config
        .bindings
        .iter()
        .filter(|binding| !bound_keys.contains(*binding))
        .cloned()
        .collect::<Vec<_>>();

      if bindings.is_empty() {
        continue;
      }

      layer_keys.extend(bindings.iter().cloned());
      resolved.push(KeybindingConfig {
        bindings,
        commands: keybinding_config.commands.clone(),
      });
    }

    bound_keys.extend(layer_keys);
  }

  resolved
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p wm user_config`
Expected: 7 passed.

- [ ] **Step 6: Document `inherit` in the sample config**

In `resources/assets/sample-config.yaml`, directly above the top-level `binding_modes:` key, insert:

```yaml
# Binding modes swap in a different set of keybindings while enabled.
# By default a mode replaces all other keybindings. Set `inherit: true`
# to override only the keys the mode binds, and let every other key fall
# through to the layer below (another inheriting mode, or `keybindings`).
```

- [ ] **Step 7: Gates and commit**

Run: `cargo fmt && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test -p wm`
Expected: all pass.

```bash
git add packages/wm-common/src/parsed_config.rs packages/wm/src/user_config.rs resources/assets/sample-config.yaml
git commit -m "feat: add inheriting binding modes"
```

---

### Task 2: Binding mode stack semantics

**Files:**
- Modify: `packages/wm/src/commands/general/enable_binding_mode.rs`
- Modify: `packages/wm/src/commands/general/disable_binding_mode.rs`

**Interfaces:**
- Consumes: `BindingModeConfig.inherit` (Task 1).
- Produces: `fn push_binding_mode(stack: &mut Vec<BindingModeConfig>, mode: BindingModeConfig)` (private to `enable_binding_mode.rs`).
- Produces: `fn remove_binding_mode(stack: &mut Vec<BindingModeConfig>, name: &str)` (private to `disable_binding_mode.rs`).
- Unchanged public: `enable_binding_mode(name: &str, state: &mut WmState, config: &UserConfig) -> anyhow::Result<()>` and `disable_binding_mode(name: &str, state: &mut WmState)`.

- [ ] **Step 1: Write the failing tests**

Append to `packages/wm/src/commands/general/enable_binding_mode.rs`:

```rs
#[cfg(test)]
mod tests {
  use wm_common::BindingModeConfig;

  use super::push_binding_mode;

  /// Creates a binding mode with no keybindings.
  fn mode(name: &str, inherit: bool) -> BindingModeConfig {
    BindingModeConfig {
      name: name.to_string(),
      display_name: None,
      inherit,
      keybindings: vec![],
    }
  }

  /// Gets the names of the modes in a stack, bottom to top.
  fn names(stack: &[BindingModeConfig]) -> Vec<&str> {
    stack.iter().map(|mode| mode.name.as_str()).collect()
  }

  #[test]
  fn non_inheriting_modes_replace_each_other() {
    let mut stack = vec![];
    push_binding_mode(&mut stack, mode("resize", false));
    push_binding_mode(&mut stack, mode("move", false));

    assert_eq!(names(&stack), ["move"]);
  }

  #[test]
  fn pushing_over_inheriting_mode_keeps_it() {
    let mut stack = vec![];
    push_binding_mode(&mut stack, mode("bank-b", true));
    push_binding_mode(&mut stack, mode("legend-b", false));

    assert_eq!(names(&stack), ["bank-b", "legend-b"]);
  }

  #[test]
  fn reenabling_moves_mode_to_top() {
    let mut stack = vec![];
    push_binding_mode(&mut stack, mode("a", true));
    push_binding_mode(&mut stack, mode("b", true));
    push_binding_mode(&mut stack, mode("a", true));

    assert_eq!(names(&stack), ["b", "a"]);
  }

  #[test]
  fn enabling_replaces_only_non_inheriting_top() {
    let mut stack = vec![];
    push_binding_mode(&mut stack, mode("bank-b", true));
    push_binding_mode(&mut stack, mode("legend", false));
    push_binding_mode(&mut stack, mode("legend-b", false));

    assert_eq!(names(&stack), ["bank-b", "legend-b"]);
  }

  #[test]
  fn inheriting_mode_replaces_non_inheriting_top() {
    let mut stack = vec![];
    push_binding_mode(&mut stack, mode("bank-b", true));
    push_binding_mode(&mut stack, mode("legend", false));
    push_binding_mode(&mut stack, mode("bank-c", true));

    assert_eq!(names(&stack), ["bank-b", "bank-c"]);
  }
}
```

Append to `packages/wm/src/commands/general/disable_binding_mode.rs`:

```rs
#[cfg(test)]
mod tests {
  use wm_common::BindingModeConfig;

  use super::remove_binding_mode;

  /// Creates a binding mode with no keybindings.
  fn mode(name: &str, inherit: bool) -> BindingModeConfig {
    BindingModeConfig {
      name: name.to_string(),
      display_name: None,
      inherit,
      keybindings: vec![],
    }
  }

  #[test]
  fn removes_mode_from_middle_of_stack() {
    let mut stack = vec![
      mode("bank-b", true),
      mode("bank-c", true),
      mode("legend", false),
    ];
    remove_binding_mode(&mut stack, "bank-c");

    let names = stack
      .iter()
      .map(|mode| mode.name.as_str())
      .collect::<Vec<_>>();
    assert_eq!(names, ["bank-b", "legend"]);
  }

  #[test]
  fn removing_inactive_mode_is_a_no_op() {
    let mut stack = vec![mode("bank-b", true)];
    remove_binding_mode(&mut stack, "legend");

    assert_eq!(stack.len(), 1);
  }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p wm binding_mode`
Expected: compile errors `cannot find function push_binding_mode` / `remove_binding_mode`.

- [ ] **Step 3: Implement `enable_binding_mode.rs`**

Replace the file contents above the test module with:

```rs
use anyhow::Context;
use wm_common::{BindingModeConfig, WmEvent};

use crate::{user_config::UserConfig, wm_state::WmState};

/// Enables the binding mode with the given name.
///
/// Non-inheriting modes on top of the stack are replaced, while
/// inheriting modes stay beneath the newly enabled mode. Re-enabling an
/// active mode moves it to the top.
pub fn enable_binding_mode(
  name: &str,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let binding_mode = config
    .value
    .binding_modes
    .iter()
    .find(|config| name == config.name)
    .with_context(|| {
      format!("No binding mode found with the name '{name}'.")
    })?;

  push_binding_mode(&mut state.binding_modes, binding_mode.clone());

  state.emit_event(WmEvent::BindingModesChanged {
    new_binding_modes: state.binding_modes.clone(),
  });

  Ok(())
}

/// Pushes a binding mode onto the top of the binding mode stack.
///
/// Removes any existing entry for the mode, then pops non-inheriting
/// modes off the top. Every mode below the top therefore inherits.
fn push_binding_mode(
  stack: &mut Vec<BindingModeConfig>,
  mode: BindingModeConfig,
) {
  stack.retain(|active| active.name != mode.name);

  while stack.last().is_some_and(|top| !top.inherit) {
    stack.pop();
  }

  stack.push(mode);
}
```

- [ ] **Step 4: Implement `disable_binding_mode.rs`**

Replace the file contents above the test module with:

```rs
use wm_common::{BindingModeConfig, WmEvent};

use crate::wm_state::WmState;

/// Disables the binding mode with the given name, wherever it is in the
/// binding mode stack.
pub fn disable_binding_mode(name: &str, state: &mut WmState) {
  remove_binding_mode(&mut state.binding_modes, name);

  state.emit_event(WmEvent::BindingModesChanged {
    new_binding_modes: state.binding_modes.clone(),
  });
}

/// Removes the binding mode with the given name from the stack.
fn remove_binding_mode(stack: &mut Vec<BindingModeConfig>, name: &str) {
  stack.retain(|mode| mode.name != name);
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p wm binding_mode`
Expected: 7 passed.

- [ ] **Step 6: Gates and commit**

Run: `cargo fmt && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test -p wm`

```bash
git add packages/wm/src/commands/general/enable_binding_mode.rs packages/wm/src/commands/general/disable_binding_mode.rs
git commit -m "feat: keep inheriting binding modes beneath newly enabled modes"
```

---

### Task 3: Workspace `binding_mode` field and validation

**Files:**
- Modify: `packages/wm-common/src/parsed_config.rs` (`WorkspaceConfig`)
- Modify: `packages/wm/src/user_config.rs` (`validate`, `linked_binding_mode_names`, `binding_mode_by_name`, `mock` validates, tests)
- Modify: `packages/wm/src/test_utils.rs` (`Workspace::mock` gains `binding_mode`)
- Modify: `packages/wm/src/commands/workspace/update_workspace_config.rs` (preserve the field)
- Modify: `packages/wm/src/commands/general/enable_binding_mode.rs` (use `binding_mode_by_name`)
- Modify: `resources/assets/sample-config.yaml` (document the field)

**Interfaces:**
- Consumes: `BindingModeConfig.inherit` (Task 1), `UserConfig::mock` (Task 1).
- Produces: `WorkspaceConfig.binding_mode: Option<String>`.
- Produces: `pub fn UserConfig::binding_mode_by_name(&self, name: &str) -> Option<&BindingModeConfig>`.
- Produces: `pub fn UserConfig::linked_binding_mode_names(&self) -> Vec<&str>`.
- Produces: `fn UserConfig::validate(config: &ParsedConfig) -> anyhow::Result<()>` (private associated fn).
- Produces: `Workspace::mock().binding_mode(String)` builder setter.

- [ ] **Step 1: Add the field**

In `packages/wm-common/src/parsed_config.rs`, inside `WorkspaceConfig`, after `keep_alive`:

```rs

  /// Binding mode that is active whenever this workspace is focused. Must
  /// name a binding mode with `inherit: true`.
  #[serde(default)]
  pub binding_mode: Option<String>,
```

- [ ] **Step 2: Fix the struct literals so the crate compiles**

In `packages/wm/src/test_utils.rs`, `Workspace::mock`: add the parameter after `display_name: Option<String>,`:

```rs
    binding_mode: Option<String>,
```

and change the literal to:

```rs
    let config = WorkspaceConfig {
      name,
      display_name,
      bind_to_monitor: None,
      keep_alive: false,
      binding_mode,
    };
```

In `packages/wm/src/commands/workspace/update_workspace_config.rs`, add to the `WorkspaceConfig { … }` literal after `keep_alive`:

```rs
    binding_mode: current_config.binding_mode.clone(),
```

- [ ] **Step 3: Write the failing tests**

In `packages/wm/src/user_config.rs`, append to the end of the `CONFIG` constant in the test module (after the `legend` mode):

```yaml

workspaces:
  - name: '1'
  - name: '2'
  - name: 'B1'
    binding_mode: 'bank-b'
  - name: 'B2'
    binding_mode: 'bank-b'
```

Add `ParsedConfig` and `WorkspaceConfig` to the test module's `wm_common` import, then add these tests:

```rs
  /// Validates a raw YAML config.
  fn validate(config_str: &str) -> anyhow::Result<()> {
    let value: ParsedConfig =
      serde_yaml::from_str(config_str).expect("Invalid YAML config.");

    UserConfig::validate(&value)
  }

  #[test]
  fn validate_accepts_inheriting_workspace_binding_mode() {
    assert!(validate(CONFIG).is_ok());
  }

  #[test]
  fn validate_rejects_unknown_workspace_binding_mode() {
    let err = validate(
      "workspaces:\n  - name: 'B1'\n    binding_mode: 'bank-z'\n",
    )
    .expect_err("Unknown binding mode should be rejected.");

    assert!(err.to_string().contains("bank-z"));
  }

  #[test]
  fn validate_rejects_non_inheriting_workspace_binding_mode() {
    let err = validate(
      "binding_modes:\n  - name: 'legend'\nworkspaces:\n  - name: 'B1'\n    binding_mode: 'legend'\n",
    )
    .expect_err("Non-inheriting binding mode should be rejected.");

    assert!(err.to_string().contains("inherit"));
  }

  #[test]
  fn linked_binding_mode_names_are_deduplicated() {
    let config = UserConfig::mock(CONFIG);

    assert_eq!(config.linked_binding_mode_names(), ["bank-b"]);
  }

  #[test]
  fn binding_mode_by_name_finds_configured_mode() {
    let config = UserConfig::mock(CONFIG);

    assert!(config.binding_mode_by_name("bank-b").is_some());
    assert!(config.binding_mode_by_name("bank-z").is_none());
  }

  #[test]
  fn workspace_binding_mode_deserializes() {
    let workspace: WorkspaceConfig =
      serde_yaml::from_str("{ name: 'B1', binding_mode: 'bank-b' }")
        .expect("Invalid workspace config.");

    assert_eq!(workspace.binding_mode.as_deref(), Some("bank-b"));
  }
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p wm user_config`
Expected: compile errors for `UserConfig::validate`, `linked_binding_mode_names` and `binding_mode_by_name`.

- [ ] **Step 5: Implement validation and the helpers**

In `packages/wm/src/user_config.rs`:

1. Add `BindingModeConfig` to the top-level `use wm_common::{…}` import.
2. In `fn read`, replace `let config_value = serde_yaml::from_str(&config_str)?;` with:

```rs
    let config_value = serde_yaml::from_str(&config_str)?;
    Self::validate(&config_value)?;
```

3. Add inside `impl UserConfig`, after `reload`:

```rs
  /// Validates references between sections of the parsed config.
  ///
  /// Each workspace `binding_mode` must name a configured binding mode
  /// that has `inherit: true`.
  fn validate(config: &ParsedConfig) -> anyhow::Result<()> {
    for workspace in &config.workspaces {
      let Some(mode_name) = &workspace.binding_mode else {
        continue;
      };

      let mode = config
        .binding_modes
        .iter()
        .find(|mode| &mode.name == mode_name)
        .with_context(|| {
          format!(
            "Workspace '{}' has binding mode '{mode_name}', but no binding mode with that name exists.",
            workspace.name
          )
        })?;

      if !mode.inherit {
        anyhow::bail!(
          "Workspace '{}' has binding mode '{mode_name}', which must set `inherit: true`.",
          workspace.name
        );
      }
    }

    Ok(())
  }

  /// Gets a configured binding mode by its name.
  pub fn binding_mode_by_name(
    &self,
    name: &str,
  ) -> Option<&BindingModeConfig> {
    self.value.binding_modes.iter().find(|mode| mode.name == name)
  }

  /// Names of the binding modes linked to a workspace via its
  /// `binding_mode`, without duplicates.
  pub fn linked_binding_mode_names(&self) -> Vec<&str> {
    let mut names = Vec::new();

    for name in self
      .value
      .workspaces
      .iter()
      .filter_map(|workspace| workspace.binding_mode.as_deref())
    {
      if !names.contains(&name) {
        names.push(name);
      }
    }

    names
  }
```

4. In `UserConfig::mock`, after parsing `value`, add:

```rs
    Self::validate(&value).expect("Invalid mock config.");
```

and add to its doc: `/// Panics if the YAML is not a valid config, or fails validation.` (replacing the existing `# Panics` line).

5. In `packages/wm/src/commands/general/enable_binding_mode.rs`, replace the lookup with:

```rs
  let binding_mode =
    config.binding_mode_by_name(name).with_context(|| {
      format!("No binding mode found with the name '{name}'.")
    })?;
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p wm`
Expected: all pass, including 6 new tests.

- [ ] **Step 7: Document the field in the sample config**

In `resources/assets/sample-config.yaml`, directly above `workspaces:`, insert:

```yaml
# Workspaces can set `binding_mode` to a binding mode with `inherit: true`.
# That mode is enabled whenever the workspace is focused, and disabled when
# focus moves to a workspace without it. Useful for giving a group of
# workspaces its own number keys.
```

- [ ] **Step 8: Gates and commit**

Run: `cargo fmt && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test -p wm`

```bash
git add packages/wm-common/src/parsed_config.rs packages/wm/src/user_config.rs packages/wm/src/test_utils.rs packages/wm/src/commands/workspace/update_workspace_config.rs packages/wm/src/commands/general/enable_binding_mode.rs resources/assets/sample-config.yaml
git commit -m "feat: allow workspaces to link a binding mode"
```

---

### Task 4: Sync linked binding modes with the focused workspace

**Files:**
- Create: `packages/wm/src/commands/general/sync_workspace_binding_mode.rs`
- Modify: `packages/wm/src/commands/general/mod.rs`
- Modify: `packages/wm/src/wm_state.rs` (two fields, `populate` call)
- Modify: `packages/wm/src/wm.rs` (`process_event`, `process_commands`)
- Modify: `packages/wm/src/commands/general/reload_config.rs`

**Interfaces:**
- Consumes: `UserConfig::binding_mode_by_name`, `UserConfig::linked_binding_mode_names` (Task 3); `enable_binding_mode`, `disable_binding_mode` (Task 2); `Workspace::mock().binding_mode(..)` (Task 3).
- Produces: `pub fn sync_workspace_binding_mode(state: &mut WmState, config: &UserConfig)`.
- Produces: `WmState.last_synced_workspace: Option<String>` and `WmState.workspace_focus_history: Vec<String>` (most recent first, no duplicates).

- [ ] **Step 1: Add the state fields**

In `packages/wm/src/wm_state.rs`, inside `pub struct WmState`, after `binding_modes`:

```rs
  /// Name of the workspace that linked binding modes were last synced
  /// against. `None` forces a re-sync on the next call to
  /// `sync_workspace_binding_mode`.
  pub last_synced_workspace: Option<String>,

  /// Names of focused workspaces, most recent first and without
  /// duplicates.
  ///
  /// Used by the recent-workspace-by-binding-mode focus targets.
  pub workspace_focus_history: Vec<String>,
```

In `WmState::new`, after `binding_modes: Vec::new(),`:

```rs
      last_synced_workspace: None,
      workspace_focus_history: Vec::new(),
```

- [ ] **Step 2: Create the module with failing tests**

Create `packages/wm/src/commands/general/sync_workspace_binding_mode.rs`:

```rs
use wm_common::{BindingModeConfig, WmEvent};

use crate::{
  traits::CommonGetters, user_config::UserConfig, wm_state::WmState,
};

/// Keeps workspace-linked binding modes in step with the focused
/// workspace, and records it in the workspace focus history.
///
/// Only acts when the focused workspace differs from the one last synced
/// against, so a manually toggled mode persists until the next workspace
/// change.
pub fn sync_workspace_binding_mode(
  state: &mut WmState,
  config: &UserConfig,
) {
  todo!()
}

#[cfg(test)]
mod tests {
  use tokio::sync::mpsc;
  use wm_platform::Dispatcher;

  use super::sync_workspace_binding_mode;
  use crate::{
    commands::{
      container::{attach_container, set_focused_descendant},
      general::{disable_binding_mode, enable_binding_mode},
    },
    models::{Monitor, Workspace},
    saved_layouts::SavedLayouts,
    user_config::UserConfig,
    wm_state::WmState,
  };

  const CONFIG: &str = r"
binding_modes:
  - name: 'bank-b'
    inherit: true
  - name: 'legend'
  - name: 'legend-b'

workspaces:
  - name: '1'
  - name: 'B1'
    binding_mode: 'bank-b'
  - name: 'B2'
    binding_mode: 'bank-b'
";

  /// Creates a `WmState` with one monitor holding workspaces `1`, `B1`
  /// and `B2`.
  fn mock_state() -> WmState {
    // A path that doesn't exist yields an empty layout store.
    let store = SavedLayouts::load(
      std::env::temp_dir()
        .join(format!("glazewm-banks-{}", uuid::Uuid::new_v4()))
        .join("layouts.yaml"),
    );

    let (event_tx, _) = mpsc::unbounded_channel();
    let (exit_tx, _) = mpsc::unbounded_channel();
    let state = WmState::new(Dispatcher::mock(), event_tx, exit_tx, store);

    let monitor = Monitor::mock()
      .workspaces(vec![
        Workspace::mock().name("1".to_string()).call(),
        Workspace::mock()
          .name("B1".to_string())
          .binding_mode("bank-b".to_string())
          .call(),
        Workspace::mock()
          .name("B2".to_string())
          .binding_mode("bank-b".to_string())
          .call(),
      ])
      .call();

    attach_container(
      &monitor.into(),
      &state.root_container.clone().into(),
      None,
    )
    .expect("Failed to attach monitor.");

    state
  }

  /// Focuses the named workspace and runs the sync.
  fn focus_and_sync(state: &mut WmState, config: &UserConfig, name: &str) {
    let workspace =
      state.workspace_by_name(name).expect("Unknown workspace.");
    set_focused_descendant(&workspace.into(), None);
    sync_workspace_binding_mode(state, config);
  }

  /// Gets the names of the active binding modes, bottom to top.
  fn stack(state: &WmState) -> Vec<&str> {
    state
      .binding_modes
      .iter()
      .map(|mode| mode.name.as_str())
      .collect()
  }

  #[test]
  fn focusing_linked_workspace_enables_its_mode() {
    let config = UserConfig::mock(CONFIG);
    let mut state = mock_state();

    focus_and_sync(&mut state, &config, "B1");

    assert_eq!(stack(&state), ["bank-b"]);
  }

  #[test]
  fn focusing_unlinked_workspace_disables_linked_modes() {
    let config = UserConfig::mock(CONFIG);
    let mut state = mock_state();

    focus_and_sync(&mut state, &config, "B1");
    focus_and_sync(&mut state, &config, "1");

    assert!(stack(&state).is_empty());
  }

  #[test]
  fn linked_mode_goes_beneath_open_legend() {
    let config = UserConfig::mock(CONFIG);
    let mut state = mock_state();

    focus_and_sync(&mut state, &config, "1");
    enable_binding_mode("legend", &mut state, &config)
      .expect("Failed to enable legend.");
    focus_and_sync(&mut state, &config, "B1");

    assert_eq!(stack(&state), ["bank-b", "legend"]);
  }

  #[test]
  fn unlinked_modes_are_left_alone() {
    let config = UserConfig::mock(CONFIG);
    let mut state = mock_state();

    focus_and_sync(&mut state, &config, "B1");
    enable_binding_mode("legend-b", &mut state, &config)
      .expect("Failed to enable legend-b.");
    focus_and_sync(&mut state, &config, "1");

    assert_eq!(stack(&state), ["legend-b"]);
  }

  #[test]
  fn unchanged_workspace_keeps_manual_mode() {
    let config = UserConfig::mock(CONFIG);
    let mut state = mock_state();

    focus_and_sync(&mut state, &config, "1");
    enable_binding_mode("bank-b", &mut state, &config)
      .expect("Failed to enable bank-b.");
    sync_workspace_binding_mode(&mut state, &config);

    assert_eq!(stack(&state), ["bank-b"]);
  }

  #[test]
  fn reload_resync_restores_linked_mode() {
    let config = UserConfig::mock(CONFIG);
    let mut state = mock_state();

    focus_and_sync(&mut state, &config, "B1");

    // Mirrors what `reload_config` does to the state.
    state.binding_modes = Vec::new();
    state.last_synced_workspace = None;
    sync_workspace_binding_mode(&mut state, &config);

    assert_eq!(stack(&state), ["bank-b"]);
  }

  #[test]
  fn focus_history_is_most_recent_first_without_duplicates() {
    let config = UserConfig::mock(CONFIG);
    let mut state = mock_state();

    focus_and_sync(&mut state, &config, "1");
    focus_and_sync(&mut state, &config, "B1");
    focus_and_sync(&mut state, &config, "1");

    assert_eq!(state.workspace_focus_history, ["1", "B1"]);
  }

  #[test]
  fn bank_and_legend_round_trip() {
    let config = UserConfig::mock(CONFIG);
    let mut state = mock_state();

    focus_and_sync(&mut state, &config, "1");
    assert!(stack(&state).is_empty());

    focus_and_sync(&mut state, &config, "B1");
    assert_eq!(stack(&state), ["bank-b"]);

    enable_binding_mode("legend-b", &mut state, &config)
      .expect("Failed to enable legend-b.");
    assert_eq!(stack(&state), ["bank-b", "legend-b"]);

    focus_and_sync(&mut state, &config, "B2");
    disable_binding_mode("legend-b", &mut state);
    assert_eq!(stack(&state), ["bank-b"]);

    focus_and_sync(&mut state, &config, "1");
    assert!(stack(&state).is_empty());
  }
}
```

In `packages/wm/src/commands/general/mod.rs`, add `mod sync_workspace_binding_mode;` and `pub use sync_workspace_binding_mode::*;` in alphabetical position (after `shell_exec`).

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p wm sync_workspace_binding_mode`
Expected: 8 tests FAIL with `not yet implemented` panics.

- [ ] **Step 4: Implement the sync**

Replace the `todo!()` function in `sync_workspace_binding_mode.rs` with:

```rs
pub fn sync_workspace_binding_mode(
  state: &mut WmState,
  config: &UserConfig,
) {
  let Some(workspace) = state
    .focused_container()
    .and_then(|focused| focused.workspace())
  else {
    return;
  };

  let workspace_config = workspace.config();

  if state.last_synced_workspace.as_deref()
    == Some(workspace_config.name.as_str())
  {
    return;
  }

  state.last_synced_workspace = Some(workspace_config.name.clone());
  record_focus(&mut state.workspace_focus_history, &workspace_config.name);

  let target_mode = workspace_config
    .binding_mode
    .as_deref()
    .and_then(|name| config.binding_mode_by_name(name));

  let has_changed = reconcile_linked_modes(
    &mut state.binding_modes,
    &config.linked_binding_mode_names(),
    target_mode,
  );

  if has_changed {
    state.emit_event(WmEvent::BindingModesChanged {
      new_binding_modes: state.binding_modes.clone(),
    });
  }
}

/// Moves a workspace name to the front of the focus history.
fn record_focus(history: &mut Vec<String>, name: &str) {
  history.retain(|entry| entry != name);
  history.insert(0, name.to_string());
}

/// Removes linked modes other than `target_mode` from the stack, and
/// inserts `target_mode` at the bottom if it isn't already active.
///
/// Inserting at the bottom keeps any open non-inheriting mode (e.g. a
/// legend) on top. Linked modes always inherit, so every mode below the
/// top still inherits.
///
/// Returns whether the stack changed.
fn reconcile_linked_modes(
  stack: &mut Vec<BindingModeConfig>,
  linked_names: &[&str],
  target_mode: Option<&BindingModeConfig>,
) -> bool {
  let original_len = stack.len();

  stack.retain(|mode| {
    !linked_names.contains(&mode.name.as_str())
      || target_mode.is_some_and(|target| target.name == mode.name)
  });

  let mut has_changed = stack.len() != original_len;

  if let Some(target) = target_mode {
    if !stack.iter().any(|mode| mode.name == target.name) {
      stack.insert(0, target.clone());
      has_changed = true;
    }
  }

  has_changed
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p wm sync_workspace_binding_mode`
Expected: 8 passed.

- [ ] **Step 6: Wire the sync into the WM**

In `packages/wm/src/wm.rs`, add `sync_workspace_binding_mode` to the existing `commands::general::{…}` import. In `process_event`, directly before `if !state.is_paused && state.pending_sync.has_changes() {`, insert:

```rs
    sync_workspace_binding_mode(state, config);

```

In `process_commands`, directly before `if state.pending_sync.has_changes() {`, insert the same two lines.

In `packages/wm/src/wm_state.rs`, change the import `general::platform_sync,` to `general::{platform_sync, sync_workspace_binding_mode},` and in `populate` replace:

```rs
    platform_sync(self, config)?;
    self.has_initialized = true;
```

with:

```rs
    platform_sync(self, config)?;
    self.has_initialized = true;

    // Enable the focused workspace's linked binding mode, now that events
    // can be emitted.
    sync_workspace_binding_mode(self, config);
```

In `packages/wm/src/commands/general/reload_config.rs`, after `state.binding_modes = Vec::new();`:

```rs

  // Force linked binding modes to re-sync against the reloaded config.
  state.last_synced_workspace = None;
```

- [ ] **Step 7: Gates and commit**

Run: `cargo fmt && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test -p wm`

```bash
git add packages/wm/src/commands/general/ packages/wm/src/wm_state.rs packages/wm/src/wm.rs
git commit -m "feat: sync workspace-linked binding modes with focus"
```

---

### Task 5: Recent-workspace-by-binding-mode focus targets

**Files:**
- Modify: `packages/wm-common/src/app_command.rs` (`InvokeFocusCommand`, tests)
- Modify: `packages/wm/src/models/workspace_target.rs`
- Modify: `packages/wm/src/user_config.rs` (`recent_workspace_with_binding_mode`, tests)
- Modify: `packages/wm/src/wm_state.rs` (`workspace_by_target`)
- Modify: `packages/wm/src/wm.rs` (focus dispatch)

**Interfaces:**
- Consumes: `WmState.workspace_focus_history` (Task 4), `WorkspaceConfig.binding_mode` (Task 3).
- Produces: `InvokeFocusCommand.recent_workspace_with_mode: Option<String>` (`--recent-workspace-with-mode <NAME>`) and `InvokeFocusCommand.recent_workspace_without_mode: bool` (`--recent-workspace-without-mode`).
- Produces: `WorkspaceTarget::RecentWithBindingMode(Option<String>)`.
- Produces: `pub fn UserConfig::recent_workspace_with_binding_mode(&self, focus_history: &[String], binding_mode: Option<&str>) -> Option<String>`.

- [ ] **Step 1: Write the failing CLI tests**

`packages/wm-common/src/app_command.rs` already ends with a `#[cfg(test)] mod tests` that has a `parse(input: &str) -> InvokeCommand` helper (it panics on parse failure). Add these tests **inside that existing module**. Do not create a second `mod tests`.

```rs
  #[test]
  fn parses_recent_workspace_with_mode() {
    let InvokeCommand::Focus(args) =
      parse("focus --recent-workspace-with-mode bank-b")
    else {
      panic!("Expected a focus command.");
    };

    assert_eq!(args.recent_workspace_with_mode.as_deref(), Some("bank-b"));
  }

  #[test]
  fn parses_recent_workspace_without_mode() {
    let InvokeCommand::Focus(args) =
      parse("focus --recent-workspace-without-mode")
    else {
      panic!("Expected a focus command.");
    };

    assert!(args.recent_workspace_without_mode);
  }

  #[test]
  fn rejects_combined_focus_targets() {
    let args = std::iter::once("").chain(
      "focus --recent-workspace-without-mode --recent-workspace"
        .split_whitespace(),
    );

    assert!(InvokeCommand::try_parse_from(args).is_err());
  }
```

- [ ] **Step 2: Write the failing config tests**

In the `packages/wm/src/user_config.rs` test module, add:

```rs
  /// Converts string slices to an owned focus history.
  fn history(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
  }

  #[test]
  fn recent_returns_most_recent_match() {
    let config = UserConfig::mock(CONFIG);
    let history = history(&["1", "B2", "2", "B1"]);

    assert_eq!(
      config.recent_workspace_with_binding_mode(&history, Some("bank-b")),
      Some("B2".to_string())
    );
    assert_eq!(
      config.recent_workspace_with_binding_mode(&history, None),
      Some("1".to_string())
    );
  }

  #[test]
  fn recent_falls_back_to_config_order() {
    let config = UserConfig::mock(CONFIG);

    assert_eq!(
      config.recent_workspace_with_binding_mode(&[], Some("bank-b")),
      Some("B1".to_string())
    );
    assert_eq!(
      config.recent_workspace_with_binding_mode(&history(&["B2"]), None),
      Some("1".to_string())
    );
  }

  #[test]
  fn recent_ignores_unconfigured_history_entries() {
    let config = UserConfig::mock(CONFIG);

    assert_eq!(
      config.recent_workspace_with_binding_mode(
        &history(&["renamed", "B2"]),
        Some("bank-b")
      ),
      Some("B2".to_string())
    );
  }

  #[test]
  fn recent_is_none_when_nothing_matches() {
    let config = UserConfig::mock(CONFIG);

    assert_eq!(
      config.recent_workspace_with_binding_mode(&[], Some("bank-z")),
      None
    );
  }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p wm-common app_command` and `cargo test -p wm user_config`
Expected: compile errors for the missing field and method.

- [ ] **Step 4: Add the CLI flags**

In `packages/wm-common/src/app_command.rs`, inside `InvokeFocusCommand`, after `recent_workspace: bool,`:

```rs

  #[clap(long)]
  pub recent_workspace_with_mode: Option<String>,

  #[clap(long)]
  pub recent_workspace_without_mode: bool,
```

- [ ] **Step 5: Add the target and the config helper**

Replace `packages/wm/src/models/workspace_target.rs` with:

```rs
use wm_platform::Direction;

pub enum WorkspaceTarget {
  Name(String),
  Recent,
  /// Most recently focused workspace whose `binding_mode` equals the
  /// given name, or that has no `binding_mode` if `None`.
  RecentWithBindingMode(Option<String>),
  NextActive,
  PreviousActive,
  NextActiveInMonitor,
  PreviousActiveInMonitor,
  Next,
  Previous,
  #[allow(dead_code)]
  Direction(Direction),
}
```

In `packages/wm/src/user_config.rs`, inside `impl UserConfig` after `linked_binding_mode_names`:

```rs
  /// Gets the name of the most recently focused workspace whose
  /// `binding_mode` equals `binding_mode` (`None` matches workspaces
  /// without one).
  ///
  /// Falls back to the first matching workspace in config order if no
  /// entry in `focus_history` matches. Returns `None` if no configured
  /// workspace matches.
  pub fn recent_workspace_with_binding_mode(
    &self,
    focus_history: &[String],
    binding_mode: Option<&str>,
  ) -> Option<String> {
    let matching = self
      .value
      .workspaces
      .iter()
      .filter(|workspace| workspace.binding_mode.as_deref() == binding_mode)
      .collect::<Vec<_>>();

    focus_history
      .iter()
      .find(|name| {
        matching.iter().any(|workspace| &workspace.name == *name)
      })
      .cloned()
      .or_else(|| matching.first().map(|workspace| workspace.name.clone()))
  }
```

- [ ] **Step 6: Resolve the target and dispatch the flags**

In `packages/wm/src/wm_state.rs`, `workspace_by_target`, add a match arm directly after the `WorkspaceTarget::Recent => (…),` arm:

```rs
      WorkspaceTarget::RecentWithBindingMode(binding_mode) => {
        let name = config
          .recent_workspace_with_binding_mode(
            &self.workspace_focus_history,
            binding_mode.as_deref(),
          )
          .with_context(|| match &binding_mode {
            Some(mode) => format!(
              "No workspace is configured with binding mode '{mode}'."
            ),
            None => {
              "No workspace is configured without a binding mode."
                .to_string()
            }
          })?;

        let workspace = self.workspace_by_name(&name);
        (Some(name), workspace)
      }
```

In `packages/wm/src/wm.rs`, in the `InvokeCommand::Focus(args)` arm, directly after the `if args.recent_workspace { … }` block:

```rs

        if let Some(binding_mode) = &args.recent_workspace_with_mode {
          focus_workspace(
            WorkspaceTarget::RecentWithBindingMode(Some(
              binding_mode.clone(),
            )),
            state,
            config,
          )?;
        }

        if args.recent_workspace_without_mode {
          focus_workspace(
            WorkspaceTarget::RecentWithBindingMode(None),
            state,
            config,
          )?;
        }
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p wm-common && cargo test -p wm`
Expected: all pass (3 new in `wm-common`, 4 new in `wm`).

- [ ] **Step 8: Gates and commit**

Run: `cargo fmt && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`

```bash
git add packages/wm-common/src/app_command.rs packages/wm/src/models/workspace_target.rs packages/wm/src/user_config.rs packages/wm/src/wm_state.rs packages/wm/src/wm.rs
git commit -m "feat: add focus targets for recent workspace by binding mode"
```

---

### Task 6: Full workspace gate

**Files:** none (verification only).

- [ ] **Step 1: Run the full CI gate set**

Run, one at a time (never backgrounded):

```shell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
```

Expected: all pass. `cargo test --workspace` includes `wm-platform`, whose harness needs `-- --test-threads=1`. If it fails only for that reason, rerun `cargo test -p wm-platform -- --test-threads=1` and record both results.

- [ ] **Step 2: Fix anything that fails, then commit any fixes**

```bash
git commit -am "fix: address workspace gate findings"
```

(Skip the commit if nothing changed.)

---

### Task 7: Personal config, legend widget and spec note

These files live outside the repo (no commit), except the spec note.

**Files:**
- Modify: `~/.glzr/glazewm/config.yaml`
- Modify: `~/.glzr/zebar/workspace-legend/widgets/legend/index.html`
- Modify: `docs/superpowers/specs/2026-10-01-workspace-banks-design.md` (§7)

**Interfaces:**
- Consumes: `inherit`, workspace `binding_mode`, `focus --recent-workspace-with-mode`, `focus --recent-workspace-without-mode` (Tasks 1–5).

- [ ] **Step 1: Back up both files**

```bash
cp ~/.glzr/glazewm/config.yaml ~/.glzr/glazewm/config.yaml.bak-20261001
cp ~/.glzr/zebar/workspace-legend/widgets/legend/index.html ~/.glzr/zebar/workspace-legend/widgets/legend/index.html.bak-20261001
```

- [ ] **Step 2: Add the bank B workspaces**

In `~/.glzr/glazewm/config.yaml`, under `workspaces:` after `- name: '9'`:

```yaml
  - name: 'B1'
    binding_mode: 'bank-b'
  - name: 'B2'
    binding_mode: 'bank-b'
  - name: 'B3'
    binding_mode: 'bank-b'
  - name: 'B4'
    binding_mode: 'bank-b'
  - name: 'B5'
    binding_mode: 'bank-b'
  - name: 'B6'
    binding_mode: 'bank-b'
  - name: 'B7'
    binding_mode: 'bank-b'
  - name: 'B8'
    binding_mode: 'bank-b'
  - name: 'B9'
    binding_mode: 'bank-b'
```

- [ ] **Step 3: Add the `bank-b` and `legend-b` modes**

In `~/.glzr/glazewm/config.yaml`, under `binding_modes:`, after the whole existing `legend` mode, at the same indentation as `- name: 'legend'`:

```yaml
  # Bank B: the number keys target workspaces B1-B9. Enabled automatically
  # while a B workspace is focused (see `binding_mode` on the workspaces).
  # Every key not listed here falls through to the normal keybindings.
  - name: 'bank-b'
    display_name: 'Bank B'
    inherit: true
    keybindings:
      - commands: ['focus --workspace B1']
        bindings: ['alt+1']
      - commands: ['focus --workspace B2']
        bindings: ['alt+2']
      - commands: ['focus --workspace B3']
        bindings: ['alt+3']
      - commands: ['focus --workspace B4']
        bindings: ['alt+4']
      - commands: ['focus --workspace B5']
        bindings: ['alt+5']
      - commands: ['focus --workspace B6']
        bindings: ['alt+6']
      - commands: ['focus --workspace B7']
        bindings: ['alt+7']
      - commands: ['focus --workspace B8']
        bindings: ['alt+8']
      - commands: ['focus --workspace B9']
        bindings: ['alt+9']
      - commands: ['move --workspace B1', 'focus --workspace B1']
        bindings: ['alt+shift+1']
      - commands: ['move --workspace B2', 'focus --workspace B2']
        bindings: ['alt+shift+2']
      - commands: ['move --workspace B3', 'focus --workspace B3']
        bindings: ['alt+shift+3']
      - commands: ['move --workspace B4', 'focus --workspace B4']
        bindings: ['alt+shift+4']
      - commands: ['move --workspace B5', 'focus --workspace B5']
        bindings: ['alt+shift+5']
      - commands: ['move --workspace B6', 'focus --workspace B6']
        bindings: ['alt+shift+6']
      - commands: ['move --workspace B7', 'focus --workspace B7']
        bindings: ['alt+shift+7']
      - commands: ['move --workspace B8', 'focus --workspace B8']
        bindings: ['alt+shift+8']
      - commands: ['move --workspace B9', 'focus --workspace B9']
        bindings: ['alt+shift+9']
      # Show the bank B legend.
      - commands: ['wm-enable-binding-mode --name legend-b']
        bindings: ['alt+0']
      # Jump back to the most recent bank A workspace.
      - commands: ['focus --recent-workspace-without-mode']
        bindings: ['alt+oem_period']

  # Bank B workspace legend. Same as `legend`, but the digits target
  # B1-B9. Closing it returns to bank B, which stays beneath it.
  - name: 'legend-b'
    display_name: 'Legend B'
    keybindings:
      - commands: ['focus --workspace B1', 'wm-disable-binding-mode --name legend-b']
        bindings: ['1']
      - commands: ['focus --workspace B2', 'wm-disable-binding-mode --name legend-b']
        bindings: ['2']
      - commands: ['focus --workspace B3', 'wm-disable-binding-mode --name legend-b']
        bindings: ['3']
      - commands: ['focus --workspace B4', 'wm-disable-binding-mode --name legend-b']
        bindings: ['4']
      - commands: ['focus --workspace B5', 'wm-disable-binding-mode --name legend-b']
        bindings: ['5']
      - commands: ['focus --workspace B6', 'wm-disable-binding-mode --name legend-b']
        bindings: ['6']
      - commands: ['focus --workspace B7', 'wm-disable-binding-mode --name legend-b']
        bindings: ['7']
      - commands: ['focus --workspace B8', 'wm-disable-binding-mode --name legend-b']
        bindings: ['8']
      - commands: ['focus --workspace B9', 'wm-disable-binding-mode --name legend-b']
        bindings: ['9']
      # Dismiss without switching.
      - commands: ['wm-disable-binding-mode --name legend-b']
        bindings: ['escape', 'enter', 'alt+0']
```

- [ ] **Step 4: Add the bank A `alt+.` binding**

In `~/.glzr/glazewm/config.yaml`, under top-level `keybindings:`, directly after the `focus --recent-workspace` (`alt+d`) entry:

```yaml

  # Jump to the most recent bank B workspace (B1 if none yet).
  - commands: ['focus --recent-workspace-with-mode bank-b']
    bindings: ['alt+oem_period']
```

- [ ] **Step 5: Make the legend widget bank-aware**

In `~/.glzr/zebar/workspace-legend/widgets/legend/index.html`:

5a. Replace the header title span

```html
        <span>Workspaces<span id="layoutTag"></span></span>
```

with

```html
        <span><span id="legendTitle">Workspaces</span><span id="layoutTag"></span></span>
```

5b. Replace

```js
      /** Binding mode that drives visibility. Must match config.yaml. */
      const MODE = 'legend';
```

with

```js
      /**
       * Binding modes that show the legend, and the workspaces each one
       * renders. Must match config.yaml.
       */
      const LEGENDS = {
        legend: {
          title: 'Workspaces',
          slots: ['1', '2', '3', '4', '5', '6', '7', '8', '9'],
        },
        'legend-b': {
          title: 'Bank B',
          slots: ['B1', 'B2', 'B3', 'B4', 'B5', 'B6', 'B7', 'B8', 'B9'],
        },
      };
```

5c. Delete the `SLOTS` constant and its doc comment:

```js
      /** Workspace slots to render, in numpad-grid order. */
      const SLOTS = ['1', '2', '3', '4', '5', '6', '7', '8', '9'];
```

5d. Add `const legendTitle = document.getElementById('legendTitle');` directly after `const modeTag = document.getElementById('modeTag');`.

5e. Replace

```js
      /** Whether the legend binding mode is currently active. */
      let modeActive = false;
```

with

```js
      /** Whether a legend binding mode is currently active. */
      let modeActive = false;

      /** Name of the active legend binding mode, if any. */
      let activeLegend = null;
```

5f. In the `binding_modes_changed` case, replace

```js
              setModeActive(modes.some(mode => mode.name === MODE));
```

with

```js
              const top = [...modes]
                .reverse()
                .find(mode => mode.name in LEGENDS);
              setActiveLegend(top?.name ?? null);
```

5g. Replace the whole `setModeActive` function with:

```js
      /** Applies a legend binding mode transition to the overlay. */
      function setActiveLegend(name) {
        if (name === activeLegend) {
          return;
        }

        activeLegend = name;
        modeActive = name !== null;

        if (modeActive) {
          if (latest) {
            render(latest);
          }

          show();
        } else {
          hide();
        }
      }
```

5h. In `render`, replace

```js
        grid.replaceChildren(
          ...SLOTS.map(slot =>
```

with

```js
        const legend = LEGENDS[activeLegend ?? 'legend'];
        legendTitle.textContent = legend.title;

        grid.replaceChildren(
          ...legend.slots.map(slot =>
```

5i. In `dismiss`, replace

```js
        if (!latest) {
          return;
        }

        try {
          await latest.runCommand(`wm-disable-binding-mode --name ${MODE}`);
```

with

```js
        if (!latest || !activeLegend) {
          return;
        }

        try {
          await latest.runCommand(
            `wm-disable-binding-mode --name ${activeLegend}`,
          );
```

5j. Confirm there are no stale references:

Run: `grep -n "MODE\b\|SLOTS\|setModeActive" ~/.glzr/zebar/workspace-legend/widgets/legend/index.html`
Expected: no output.

- [ ] **Step 6: Record the badge decision in the spec**

In `docs/superpowers/specs/2026-10-01-workspace-banks-design.md` §7, replace the "Bank indicator" bullet (both lines, including "Because of sync…") with:

```markdown
- Bank indicator: no widget change needed. The `mushfikurr.overline-zebar`
  bar already renders a chip per active binding mode using
  `displayName ?? name`, so `bank-b` with `display_name: 'Bank B'` *is* the
  badge. Because of sync, it always matches the focused workspace.
```

```bash
git add docs/superpowers/specs/2026-10-01-workspace-banks-design.md
git commit -m "docs: note existing bar chip serves as the bank indicator"
```

---

### Task 8: Live verification

**Files:** none.

- [ ] **Step 1: Build and start the local WM**

```powershell
Get-Process glazewm -ErrorAction SilentlyContinue | Stop-Process -Force
Get-NetTCPConnection -LocalPort 6123 -State Listen -ErrorAction SilentlyContinue
```

Expected: no listener on 6123. If a stale listener remains, stop and ask the user to reboot (see `CLAUDE.local.md`).

```shell
cargo build --workspace
./target/debug/glazewm.exe start -v
```

Expected: starts without the config validation error. The user's config parses.

- [ ] **Step 2: Walk the live checks with the user**

Ask the user to perform each one and confirm. Record pass/fail for each:

1. `alt+.` from workspace 2 lands on B1, and the bar shows a "Bank B" chip.
2. `alt+3` lands on B3. `alt+.` returns to 2. `alt+.` again returns to B3.
3. In bank B, `alt+shift+4` moves the focused window to B4.
4. In bank B, `alt+h` and `alt+r` (resize mode) still work. Leaving resize mode returns to bank B.
5. `alt+s` from 9, with a B workspace active, flips the chip to "Bank B".
6. Activating a window that lives on a B workspace from the taskbar flips the chip to "Bank B".
7. `alt+0` in bank B shows the "Bank B" legend. Pressing `3` lands on B3, and the chip still shows "Bank B".
8. `alt+0` in bank A shows the normal legend, unchanged.
9. Quit with the B workspace focused, then restart. The "Bank B" chip shows immediately after start.
10. `alt+shift+r` (reload config) while on B3 keeps "Bank B".

- [ ] **Step 3: Report**

Summarise results to the user, with logs for any failure. Do not claim success for unconfirmed checks.
