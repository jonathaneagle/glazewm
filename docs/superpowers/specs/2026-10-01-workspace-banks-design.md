# Workspace banks (inheriting binding modes) — design

Date: 2026-10-01
Status: approved in conversation, spec awaiting review
Branch: `feat/workspace-banks` (from `main`)

## Problem

The office layout fills all nine workspaces, and many apps there need the
full screen. Adding more workspaces needs more hotkeys, but `alt+shift+1..9`
already moves windows and `ctrl+alt+1..9` is likely to clash with other apps.

The idea is a second bank of nine workspaces (`B1`–`B9`) on the same number
keys, with `alt+.` toggling between banks.

GlazeWM's binding modes can almost express this, but two things block it:

1. **A mode replaces every keybinding.** `active_keybinding_configs`
   (`wm/src/user_config.rs:358`) uses the first mode's keybindings *instead
   of* the base `keybindings`. A `bank-b` mode would have to copy all ~60
   base bindings just to change 18 of them, and every later keybinding edit
   would have to be made twice.
2. **Only one mode can be active.** `enable_binding_mode`
   (`wm/src/commands/general/enable_binding_mode.rs:20`) sets
   `state.binding_modes = vec![mode]`. Opening the `alt+0` legend from bank B
   discards `bank-b`, so you silently drop back into bank A.

## Goals

- A binding mode can override some keys and leave the rest to the layers
  underneath it.
- An inheriting mode survives while another mode is opened on top of it.
- Existing configs (no `inherit` anywhere) behave exactly as they do today.
- The active bank always matches the focused workspace, however focus got
  there (hotkey, `focus --next-active-workspace`, notification, Zebar click).
- Personal config: bank B (`B1`–`B9`), with `alt+.` jumping to the most
  recent workspace in the other bank, plus a bank-aware legend and a visible
  bank indicator.

## Non-goals

- **A first-class "bank" concept in the WM.** Banks are just config built on
  generic features: inheriting modes, and modes linked to workspaces. That keeps the WM change small and
  upstreamable.
- **Monocle (one-window-at-a-time) layout.** Agreed as the follow-up, with
  its own spec.
- **More than two banks.** The mechanism allows it (`bank-c` etc.), but only
  bank B is configured.

## Design

### 1. `inherit` flag on `BindingModeConfig`

`wm-common/src/parsed_config.rs`:

```rs
/// Whether keys this mode does not bind fall through to the layer below
/// (the next active binding mode, or the base `keybindings`).
#[serde(default)]
pub inherit: bool,
```

The default is `false`, so existing configs are unchanged. The field is also
serialized (camelCase) in `BindingModesChanged` and the `binding-modes`
query, so clients can see it.

### 2. Binding-mode stack semantics

`state.binding_modes` becomes an ordered stack: index 0 is the bottom and the
last element is the top (the active mode).

**Enable `M`** (`enable_binding_mode`):

1. Look up `M` in config (error if unknown, as today).
2. Remove `M` from the stack if present.
3. Pop modes off the top while the top has `inherit: false`.
4. Push `M`.

Invariant: every mode below the top has `inherit: true`, so a non-inheriting
mode can only ever be the top. No mode appears twice, so the stack is never
deeper than the number of configured modes.

**Disable `M`** (`disable_binding_mode`): remove `M` by name, wherever it is
in the stack. This is unchanged.

**Compatibility:** without `inherit`, step 3 always empties the stack before
the push, which is exactly today's `vec![mode]`. Disable is unchanged too.

**Worked example:** start on workspace 2 with `[]` (bank A).

| Action | Stack after |
|---|---|
| `alt+.` focuses B1; sync (§4) adds `bank-b` | `[bank-b]` |
| `alt+0` enables `legend-b` | `[bank-b, legend-b]` |
| `3` focuses B3, disables `legend-b` | `[bank-b]` |
| `alt+.` focuses 2; sync removes `bank-b` | `[]` |

### 3. Keybinding resolution

`UserConfig::active_keybinding_configs(binding_modes, is_paused)` walks the
stack from the top:

1. Yield the top mode's keybindings.
2. If that mode has `inherit: true`, continue with the next mode down.
   Otherwise stop.
3. If every mode inherits (or the stack is empty), finish with the base
   `keybindings`.

**Shadowing:** a keybinding from a lower layer is dropped if any of its
`bindings` keys is already bound by a higher layer. Its other keys are kept,
by filtering the `KeybindingConfig.bindings` list. This keeps the two existing
consumers correct:

- `wm.rs:99` takes the first config that contains the pressed key. Higher
  layers come first, so overrides win.
- `main.rs:274` registers the flattened `bindings` with the keyboard hook.
  Shadowed duplicates are already removed, so each key is registered once.

The pause filter (only `WmTogglePause` while paused) applies after
resolution, unchanged. "Active mode" changes from `binding_modes.first()` to the top
(`last()`). With today's single-element stack these are the same.

### 4. Workspace-linked binding modes (bank follows focus)

`WorkspaceConfig` (`wm-common/src/parsed_config.rs`) gains:

```rs
/// Binding mode that is active whenever this workspace is focused.
#[serde(default)]
pub binding_mode: Option<String>,
```

A mode named by any workspace is a **linked mode**.

**Validation** (on config load and reload): `binding_mode` must name a
configured binding mode with `inherit: true`. Otherwise config parsing fails
with a clear message. A non-inheriting linked mode would disable every other
key, which is never what's intended.

**Sync** — new `sync_workspace_binding_mode(state, config)` in
`wm/src/commands/general/`. It is called at the end of both
`WindowManager::process_event` and `process_commands` (`wm.rs:146`,
`wm.rs:178`), just before the `platform_sync` check. That covers every path
that can change focus: commands, keybindings, IPC, and external focus events
(`handle_window_focused`).

1. Resolve the focused workspace. If its name equals
   `state.last_synced_workspace` (new `Option<String>` on `WmState`), return.
   Syncing only on workspace *changes* means a manually toggled mode isn't
   undone every iteration.
2. Record the name in `state.last_synced_workspace` and move it to the front
   of `state.workspace_focus_history` (new MRU `Vec<String>`, deduplicated, so
   bounded by the number of configured workspaces).
3. Let `W` be the focused workspace's `binding_mode`. Remove every linked
   mode other than `W` from the stack. If `W` is set and not on the stack,
   insert it at index 0, beneath any open non-inheriting mode such as a
   legend. This keeps the §2 invariant, because `W` inherits.
4. Emit `BindingModesChanged` only if the stack actually changed.

Config reload resets `last_synced_workspace` to `None`, so the next iteration
re-syncs against the fresh (cleared) stack. Workspace focus history is kept
across reloads; names that no longer exist are skipped on lookup.

**Behaviour when focus changes while a legend is open:** for example, a
notification click while bank A's `legend` is showing. The stack becomes
`[bank-b, legend]`, so the legend stays open with bank-A digits. Picking a
digit focuses that workspace and sync drops `bank-b` again. That's consistent
with the rule, and an unusual case, so it's accepted.

### 5. "Recent workspace in other bank" focus targets

`alt+.` jumps rather than flipping keys. Two new mutually exclusive flags join
the `InvokeFocusCommand` group (`wm-common/src/app_command.rs:311`):

- `--recent-workspace-with-mode <NAME>`: the most recently focused workspace
  whose `binding_mode` is `NAME`.
- `--recent-workspace-without-mode`: the most recently focused workspace with
  no `binding_mode`.

They resolve through a new `WorkspaceTarget` variant in `wm_state.rs`, using
`workspace_focus_history`. If no workspace in the set has been focused yet,
they fall back to the first workspace in config order that matches the set
(`B1` / `1`). If no configured workspace matches at all, they return an error.
The target may currently be inactive (empty and destroyed). Focusing it
re-activates it, exactly like `focus --workspace <name>`.

Sync (§4) then switches the mode, so the `alt+.` bindings never touch the
stack directly.

### 6. Personal config (`~/.glzr/glazewm/config.yaml`, not in repo)

- `workspaces`: append `B1`–`B9` after `9`, each with
  `binding_mode: 'bank-b'`.
- Base `keybindings`: `alt+oem_period` (`.`, per
  `wm-platform/src/models/key.rs:415`) runs
  `focus --recent-workspace-with-mode bank-b`.
- `binding_modes`:
  - `bank-b` (`inherit: true`, `display_name: 'Bank B'`):
    - `alt+1..9` focuses `B1..B9`.
    - `alt+shift+1..9` moves the window to `B1..B9`, then focuses it.
    - `alt+0` enables `legend-b`.
    - `alt+oem_period` runs `focus --recent-workspace-without-mode`.
  - `legend-b` (not inheriting): a copy of `legend` where digits focus
    `B1..B9` and then disable `legend-b`. `escape`/`enter`/`alt+0` disable
    `legend-b`.
- The existing `legend` mode is unchanged.

### 7. Zebar (`~/.glzr/zebar/workspace-legend/`, not in repo)

- The legend widget already subscribes to `binding_modes_changed`. It shows
  when `legend` **or** `legend-b` is active. When `legend-b` is active, it
  renders `B1`–`B9` in the 3×3 grid with a "Bank B" heading, and clicks focus
  the `B` workspaces.
- Bank indicator: a small "B" badge while `bank-b` is anywhere on the stack.
  Because of sync, this always matches the focused workspace.
  It goes in the same widget pack (or the bar widget if that's a better fit,
  decided at implementation time by reading the existing bar setup).

## Error handling

- Enabling an unknown mode returns the existing `No binding mode found` error.
- Disabling a mode that isn't active stays a no-op, as today.
- Config reload clears the stack (`reload_config.rs:75`, unchanged), so a
  mode removed from config can't linger.
- A workspace `binding_mode` that is unknown or non-inheriting fails config
  validation (§4).
- The recent-workspace targets error if no configured workspace matches the
  requested set (§5).

## Testing

Unit tests (`#[cfg(test)]`):

- `user_config.rs` resolution:
  - Empty stack gives the base bindings.
  - A non-inheriting mode gives only its own bindings.
  - An inheriting mode overrides a base key and keeps the other base keys.
  - Partial shadowing keeps a lower config's unshadowed keys.
  - Pause filtering still works with an inheriting stack.
- Enable/disable stack:
  - Replace semantics without `inherit` (compatibility).
  - Pushing over an inheriting mode.
  - Re-enabling moves the mode to the top without duplicating it.
  - Enabling a non-inheriting mode pops the non-inheriting top.
  - Disabling a mode in the middle of the stack.
  - The worked-example sequence above.
- Workspace sync (using the `test_utils` mock builders):
  - Focusing a linked workspace adds its mode at index 0, under an open
    legend.
  - Focusing an unlinked workspace removes linked modes and leaves unlinked
    modes alone.
  - No change to the focused workspace means no change to the stack and no
    event, so a manual toggle survives.
  - Focus history is MRU and deduplicated.
  - Validation rejects an unknown or non-inheriting `binding_mode`.
- Recent-workspace targets:
  - Returns the MRU match.
  - Falls back to config order when there's no history.
  - Errors when nothing matches.

Gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
warnings`, `cargo test -p wm`, `cargo test -p wm-common`.

Live check (stop installed GlazeWM first):

- `alt+.` jumps between banks, back to the last-used workspace in each, and
  the badge follows.
- `alt+s` cycling from 9 into an active `B` workspace flips to bank B keys.
- Activating a window on a `B` workspace from the taskbar flips to bank B
  keys.
- `alt+3` lands on B3 in bank B.
- `alt+shift+4` moves the window to B4.
- Non-number keys (`alt+h`, resize) still work in bank B.
- `alt+0` in bank B shows the Bank B legend, and picking a workspace returns
  you to bank B.
- Bank A legend behaviour is unchanged.

## Known side effects

- `focus --next-active-workspace` / `--prev-active-workspace` cycle through
  active `B` workspaces too, in config order after `9`. This is intended: sync
  keeps the keys matching wherever you land.
- `focus --recent-workspace` (`alt+d`) can cross banks. Sync handles it in the
  same way.
- Clients that assumed `bindingModes` has at most one element may now see
  two. The repo's own consumer (legend widget) uses `.some(...)`, so it's
  safe.
