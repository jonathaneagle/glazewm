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
- Personal config: bank B (`B1`–`B9`) on `alt+.`, plus a bank-aware legend
  and a visible bank indicator.

## Non-goals

- **A first-class "bank" concept in the WM.** Banks are just config built on
  a generic binding-mode feature. That keeps the WM change small and
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

**Worked example:** start with `[]` (bank A).

| Action | Stack after |
|---|---|
| `alt+.` enables `bank-b` | `[bank-b]` |
| `alt+0` enables `legend-b` | `[bank-b, legend-b]` |
| `3` focuses B3, disables `legend-b` | `[bank-b]` |
| `alt+.` disables `bank-b` | `[]` |

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

### 4. Personal config (`~/.glzr/glazewm/config.yaml`, not in repo)

- `workspaces`: append `B1`–`B9` after `9`.
- Base `keybindings`: `alt+oem_period` (`.`, per `wm-platform/src/models/key.rs:415`) enables `bank-b`.
- `binding_modes`:
  - `bank-b` (`inherit: true`, `display_name: 'Bank B'`):
    - `alt+1..9` focuses `B1..B9`.
    - `alt+shift+1..9` moves the window to `B1..B9`, then focuses it.
    - `alt+0` enables `legend-b`.
    - `alt+oem_period` disables `bank-b`.
  - `legend-b` (not inheriting): a copy of `legend` where digits focus
    `B1..B9` and then disable `legend-b`. `escape`/`enter`/`alt+0` disable
    `legend-b`.
- The existing `legend` mode is unchanged.

### 5. Zebar (`~/.glzr/zebar/workspace-legend/`, not in repo)

- The legend widget already subscribes to `binding_modes_changed`. It shows
  when `legend` **or** `legend-b` is active. When `legend-b` is active, it
  renders `B1`–`B9` in the 3×3 grid with a "Bank B" heading, and clicks focus
  the `B` workspaces.
- Bank indicator: a small "B" badge while `bank-b` is anywhere on the stack.
  It goes in the same widget pack (or the bar widget if that's a better fit,
  decided at implementation time by reading the existing bar setup).

## Error handling

- Enabling an unknown mode returns the existing `No binding mode found` error.
- Disabling a mode that isn't active stays a no-op, as today.
- Config reload clears the stack (`reload_config.rs:75`, unchanged), so a
  mode removed from config can't linger.

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

Gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
warnings`, `cargo test -p wm`, `cargo test -p wm-common`.

Live check (stop installed GlazeWM first):

- `alt+.` toggles banks and the badge appears/disappears.
- `alt+3` lands on B3 in bank B.
- `alt+shift+4` moves the window to B4.
- Non-number keys (`alt+h`, resize) still work in bank B.
- `alt+0` in bank B shows the Bank B legend, and picking a workspace returns
  you to bank B.
- Bank A legend behaviour is unchanged.

## Known side effects

- `focus --next-active-workspace` / `--prev-active-workspace` cycle through
  active `B` workspaces too, in config order after `9`. Accepted.
- Clients that assumed `bindingModes` has at most one element may now see
  two. The repo's own consumer (legend widget) uses `.some(...)`, so it's
  safe.
