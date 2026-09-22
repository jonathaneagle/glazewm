# Workspace layout profiles — design

Date: 2026-09-22
Status: approved, not yet implemented
Branch: `feat/workspace-layout-profiles`

## Problem

Workspaces are pinned to monitors only by position in the container tree, and
that association is lost whenever a monitor disappears.

On undock, `remove_monitor` (`wm/src/commands/monitor/remove_monitor.rs:24`)
moves every non-empty workspace to whichever other monitor it finds first. It
records nothing about where those workspaces came from. On redock, the only
thing that restores placement is `move_bounded_workspaces_to_new_monitor`
(`wm/src/commands/monitor/add_monitor.rs:42`), which acts solely on workspaces
whose config declares `bind_to_monitor`.

Two gaps follow:

1. Without `bind_to_monitor`, nothing is restored, so workspaces must be moved
   back by hand after every redock.
2. `bind_to_monitor` is a **monitor index** — a left-to-right slot, not a
   physical display. A user who docks at two locations with different screen
   arrangements cannot express "workspace 5 belongs on the Dell at the office
   and on the other Dell at home" with a single static binding.

## Goals

- Save the current workspace-to-monitor mapping under a user-chosen name.
- Restore it automatically when that same set of displays is next seen.
- Distinguish locations by display identity, so several arrangements coexist.
- Survive a reboot and the trip between locations.

## Non-goals

- **Driving Windows display settings.** Physical arrangement, primary display
  and resolution stay Windows' responsibility. Windows already remembers these
  per display-set; duplicating it risks the two fighting, and failures are
  disruptive. Explicitly rejected during design.
- **Restoring which workspace is displayed or focused per monitor.** Mapping
  only. Deferred, not refused.
- **Restoring window-to-workspace assignment.** Windows move with their
  workspace, so this is implied rather than stored.
- **Workspace ordering within a monitor.** `sort_workspaces`
  (`wm/src/commands/workspace/sort_workspaces.rs:9`) always sorts by config
  order, so stored order would be ignored. Order is a function of
  `config.yaml`, by design.
- **Upstream contribution.** This is a personal fork-in-waiting. The design
  optimises for correctness and the owner's workflow, not for what upstream
  maintainers would accept — notably, it introduces file-backed state, which
  GlazeWM currently has none of.

## Design

### Storage

A new file, `~/.glzr/glazewm/layouts.yaml`. Deliberately **not** `config.yaml`:
that file is hand-authored with comments, and programmatic writes would destroy
its formatting.

```yaml
version: 1
layouts:
  office:
    saved_at: 1758550800
    monitors:
      - hardware_id: DELA26B
        device_path: '\\?\DISPLAY#DELA26B#4&353f47b2&0&UID12613#{e6f07b5f-...}'
        workspaces: ['1', '5']
      - hardware_id: DELA269
        device_path: '\\?\DISPLAY#DELA269#4&353f47b2&0&UID8261#{e6f07b5f-...}'
        workspaces: ['2', '4', '6', '7']
      - hardware_id: AUO82B2
        device_path: '\\?\DISPLAY#AUO82B2#4&353f47b2&0&UID8388688#{e6f07b5f-...}'
        workspaces: ['3']
```

`version` allows the format to change without silently misreading old files.
Writes use write-to-temp-then-rename, so an interrupted write cannot leave a
corrupt store.

`workspaces` is stored as an ordered list purely because YAML sequences are
ordered; the order carries no meaning (see non-goals).

`saved_at` is Unix epoch seconds, and is informational only — it exists so a
human editing the file can tell layouts apart. Nothing reads it, and matching
never consults it. Epoch seconds rather than an ISO-8601 timestamp keeps the
store free of a date-time dependency, since nothing formats or parses the value.

**Platform scope.** The store and matching are written for both platforms via
the same `cfg` split the rest of the codebase uses, keyed on `device_uuid` on
macOS. Only Windows is exercised in practice, and every example in this
document is Windows. macOS correctness is by construction, not by test.

### Module

`packages/wm/src/saved_layouts.rs`, alongside `user_config.rs`. This is
WM-local runtime state, so it does not belong in `wm-common`.

Types mirror the platform split already used by `NativeMonitorProperties`
(`wm/src/models/native_monitor_properties.rs:6`): `device_path` and
`hardware_id` under `#[cfg(target_os = "windows")]`, `device_uuid` under
`#[cfg(target_os = "macos")]`. Serialised with `serde_yaml`, already a `wm`
dependency.

The loaded store lives on `WmState` next to other runtime state such as
`binding_modes`. It is loaded once during `start_wm`; a load failure is
non-fatal and yields an empty store.

### Monitor identity matching

Matching a saved monitor entry to a live monitor, in priority order. This
deliberately mirrors `find_matching_monitor`
(`wm/src/events/handle_display_settings_changed.rs:124`):

1. **`device_path` equality.** Exact, but embeds the adapter and port
   (`\\?\DISPLAY#DELA26B#4&353f47b2&0&UID12613#{...}`), so it can change when
   the same panel is attached through a different dock or port.
2. **`hardware_id` equality, only when unambiguous** — exactly one live monitor
   and exactly one saved entry carry that ID. This is what lets a layout
   survive a changed device path.
3. Otherwise unmatched.

The uniqueness guard in step 2 matters for a desk with two identical monitors:
they share a `hardware_id`, so rather than guess, matching falls back to
`device_path` or leaves them unmatched. GlazeWM's own code treats `hardware_id`
as a last resort for the same reason.

### Commands

Two new `InvokeCommand` variants, following the existing `Wm*` convention for
global commands:

```shell
glazewm command wm-save-workspace-layout --name office
glazewm command wm-restore-workspace-layout [--name office]
```

Named "workspace layout" rather than "layout" because in a tiling window
manager "layout" already denotes the tiling arrangement.

`wm-save-workspace-layout` captures every monitor's identity and the names of
its workspaces, upserts under `--name`, and writes the store. Re-saving an
existing name overwrites it, so re-saving after rearranging is the intended
way to update a layout.

`wm-restore-workspace-layout` applies a layout. With `--name`, that layout;
without, the layout whose monitor set exactly matches the current displays.

### Restore

Shared by the manual and automatic paths:

1. Resolve the layout (by name, or by exact display-set match).
2. Match each saved monitor entry to a live monitor, per the rules above.
3. For each workspace named in a matched entry: if it is currently active and
   not already on the target monitor, call `move_workspace_to_monitor`.

There is deliberately no step 4. `move_workspace_to_monitor`
(`wm/src/commands/monitor/add_monitor.rs:93`) already handles everything else:

- activates a replacement workspace if the origin monitor would be left empty
  (line 132),
- calls `sort_workspaces` on the target (line 147),
- queues redraws through `PendingSync`, sets pending DPI adjustment, recentres
  floating placements, and emits `WorkspaceUpdated`.

Restore therefore never touches the platform directly, in keeping with the
deferred-rendering rule.

### Automatic restore

Invoked at the end of `handle_display_settings_changed`, after `sort_monitors`
and after the existing `move_bounded_workspaces_to_new_monitor` pass.

Two guards:

- **Only when the monitor set changed.** `PlatformEvent::DisplaySettingsChanged`
  also fires for DPI changes and unrelated device events, as noted in the
  existing comments in that handler.
- **Exact match required** — every saved monitor present, and no extra
  monitors. A half-applied layout on an unfamiliar display set would be worse
  than doing nothing. This also provides natural debouncing: docking emits a
  burst of events, and no layout can match until every display has arrived.

Gated by a new `general.restore_workspace_layout` (default `true`) in
`GeneralConfig` (`wm-common/src/parsed_config.rs:72`), giving a kill switch if
it misbehaves.

**Interaction with `bind_to_monitor`:** the existing bound-workspace pass runs
first and this runs second, so a saved layout wins for any workspace it names.
This ordering is intentional rather than incidental.

### Refactor in scope

`move_workspace_to_monitor` currently lives in `add_monitor.rs` despite being a
general primitive used by this feature and by monitor removal. Move it to
`wm/src/commands/monitor/move_workspace_to_monitor.rs`. Pure relocation, no
behaviour change.

## Failure handling

Every failure degrades to "the feature does nothing". None may crash the WM.

| Situation | Behaviour |
|---|---|
| `layouts.yaml` missing | Empty store, silent. First save creates it. |
| Malformed YAML | Rename to `layouts.yaml.corrupt-<timestamp>`, start empty, log an error. Never silently clobber other layouts. |
| `version` newer than known | Load nothing and refuse to write, so a downgrade cannot destroy layouts. |
| Write fails (permissions, disk) | Command returns `Err`, surfaced through the existing non-fatal error path. |
| Workspace named in layout is not active | Skipped, debug log. Not force-created. |
| Saved monitor matches no live monitor | Its workspaces are left where they are. |
| Ambiguous `hardware_id` | Treated as unmatched. |
| Restore names an unknown layout | `Err` listing the available names. |

## Testing

`#[cfg(test)]` modules run via `cargo test -p wm`, using the existing
`Monitor::mock()` and `Workspace::mock()` bon builders in
`wm/src/test_utils.rs`. `wm-platform` is untouched, so its main-thread
`libtest-mimic` harness is not involved.

- **Serde round-trip of the store.** Required. An asymmetric
  serialise/deserialise bug of exactly this kind already exists in this
  codebase — `InvokeCommand` serialises as an object but deserialises only from
  a string (`wm-common/src/app_command.rs:156` and `:265`), which breaks
  `query binding-modes`. A round-trip assertion catches that class of mistake.
- **Identity matching**: `device_path` hit; unique `hardware_id` hit; ambiguous
  `hardware_id` produces no match.
- **Set matching**: exact set triggers auto-restore; subset and superset do not.
- **Restore**: given mocked monitors, workspaces and a layout, assert the final
  mapping, and assert workspaces absent from the layout are untouched.
- **Corrupt-file recovery**: invalid YAML yields an empty store plus a backup
  file, and no error to the caller.

## Manual verification

The code path can be exercised without physically docking:
`DisplaySwitch.exe /internal` then `/extend` drives the same monitor
add/remove handler. It briefly blanks and rearranges the screens, so it is run
only with the owner's agreement at the time.

Sequence: arrange screens → save layout → inspect `layouts.yaml` → simulate
undock → confirm consolidation → simulate redock → confirm restore → check
`~/.glzr/glazewm/errors.log`. The real test remains the next genuine commute
between the two desks.

## Files

**Added**

- `packages/wm/src/saved_layouts.rs`
- `packages/wm/src/commands/monitor/save_workspace_layout.rs`
- `packages/wm/src/commands/monitor/restore_workspace_layout.rs`
- `packages/wm/src/commands/monitor/move_workspace_to_monitor.rs` (relocation)

**Changed**

- `packages/wm-common/src/app_command.rs` — two `InvokeCommand` variants
- `packages/wm-common/src/parsed_config.rs` — `general.restore_workspace_layout`
- `packages/wm/src/wm.rs` — command dispatch
- `packages/wm/src/wm_state.rs` — holds the loaded store
- `packages/wm/src/main.rs` — loads the store at startup
- `packages/wm/src/events/handle_display_settings_changed.rs` — restore hook
- `packages/wm/src/commands/monitor/mod.rs` — module wiring
- `packages/wm/src/commands/monitor/add_monitor.rs` — relocated function removed

## Deferred

- **`move-workspace --monitor <index>`.** Proposed during design, then dropped:
  restore works against `move_workspace_to_monitor` internally, and manual
  recovery is covered by `wm-restore-workspace-layout`. `Focus` can already
  target a monitor (`app_command.rs:325`) while `MoveWorkspace` takes only a
  direction, so the asymmetry is worth closing eventually — just not as
  speculative scope here.
- **Restoring the displayed and focused workspace per monitor.**
- **A command to list or delete layouts.** The YAML is hand-editable.
