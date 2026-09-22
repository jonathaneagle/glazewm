# Workspace Layout Profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Save the workspace-to-monitor mapping under a named layout and restore it automatically when that same set of displays is next connected.

**Architecture:** A file-backed store (`~/.glzr/glazewm/layouts.yaml`) holds named layouts, each recording monitor identities and the workspaces assigned to them. Two new `InvokeCommand` variants save and restore. Restore resolves saved monitor identities against live monitors, then delegates each move to the existing `move_workspace_to_monitor` primitive. A hook at the end of `handle_display_settings_changed` restores automatically when the live display set exactly matches a saved layout.

**Tech Stack:** Rust (nightly), `serde` + `serde_yaml` for the store, `anyhow` for errors, `tracing` for logging, `bon` mock builders for tests. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-22-workspace-layout-profiles-design.md`

## Global Constraints

- Toolchain is nightly; `default-members` is `wm` + `wm-cli`, so use `cargo test -p wm`.
- `rustfmt.toml`: 2-space indent, **75-column max width**, `imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`.
- Workspace lints enable `clippy::pedantic` as warnings, and CI fails on any warning: `cargo clippy --all-targets --all-features -- -D warnings`.
- Use `anyhow` for errors in the `wm` crate (`crate::Error`/`crate::Result` is `wm-platform` only).
- Avoid `.unwrap()` wherever possible; use `.expect("...")` with a message in tests.
- Every function gets a doc comment. All comments end with a punctuation mark. Wrap type names in backticks.
- Use `tracing` macros for logging, never `println!`.
- **No new crate dependencies.** This is why `saved_at` is stored as Unix epoch seconds rather than the RFC3339 string shown in the spec's example — adding `chrono` for a purely informational field is not justified. This is a deliberate, documented deviation from the spec.
- Work happens on branch `feat/workspace-layout-profiles`. Commit after every task.

---

### Task 1: Expose monitor identity fields on the test mock

Identity matching cannot be tested until `NativeMonitorProperties::mock()` can produce a monitor carrying a `hardware_id` and `device_path`. It currently hardcodes both to `None`.

**Files:**
- Modify: `packages/wm/src/test_utils.rs:84-109`
- Test: `packages/wm/src/test_utils.rs` (new `#[cfg(test)]` module at end of file)

**Interfaces:**
- Consumes: nothing.
- Produces: `NativeMonitorProperties::mock()` and `Monitor::mock()` gain optional builder parameters `hardware_id: Option<String>` and `device_path: Option<String>` (Windows only), and `device_uuid: Option<String>` (macOS only).

- [ ] **Step 1: Write the failing test**

Append to `packages/wm/src/test_utils.rs`:

```rust
#[cfg(test)]
mod tests {
  use crate::models::Monitor;

  #[test]
  fn mock_monitor_carries_identity() {
    let monitor = Monitor::mock()
      .hardware_id(Some("DELA26B".to_string()))
      .device_path(Some("\\\\?\\DISPLAY#DELA26B#UID1".to_string()))
      .call();

    let properties = monitor.native_properties();

    assert_eq!(properties.hardware_id.as_deref(), Some("DELA26B"));
    assert_eq!(
      properties.device_path.as_deref(),
      Some("\\\\?\\DISPLAY#DELA26B#UID1")
    );
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p wm mock_monitor_carries_identity`
Expected: FAIL to compile — no method `hardware_id` on the builder.

- [ ] **Step 3: Add the builder parameters**

In `packages/wm/src/test_utils.rs`, change `NativeMonitorProperties::mock` to accept and use the identity fields:

```rust
#[bon]
impl NativeMonitorProperties {
  /// Creates a mock `NativeMonitorProperties` for use in tests.
  #[builder]
  pub fn mock(
    #[builder(default = String::new())] device_name: String,
    #[builder(default = mock_bounds())] bounds: Rect,
    #[builder(default = mock_working_area())] working_area: Rect,
    #[builder(default = MOCK_DPI)] dpi: u32,
    #[builder(default = MOCK_SCALE_FACTOR)] scale_factor: f32,
    #[cfg(target_os = "windows")] hardware_id: Option<String>,
    #[cfg(target_os = "windows")] device_path: Option<String>,
    #[cfg(target_os = "macos")] device_uuid: Option<String>,
  ) -> Self {
    Self {
      device_name,
      bounds,
      working_area,
      dpi,
      scale_factor,
      #[cfg(target_os = "macos")]
      device_uuid: device_uuid.unwrap_or_default(),
      #[cfg(target_os = "windows")]
      handle: 0,
      #[cfg(target_os = "windows")]
      hardware_id,
      #[cfg(target_os = "windows")]
      device_path,
    }
  }
}
```

Then forward them from `Monitor::mock`:

```rust
#[bon]
impl Monitor {
  /// Creates a mock `Monitor` for use in tests.
  #[builder]
  pub fn mock(
    #[builder(default = String::new())] device_name: String,
    #[builder(default = mock_bounds())] bounds: Rect,
    #[builder(default = mock_working_area())] working_area: Rect,
    #[builder(default = MOCK_DPI)] dpi: u32,
    #[builder(default = MOCK_SCALE_FACTOR)] scale_factor: f32,
    #[builder(default = Display::mock())] native: Display,
    #[builder(default = vec![])] workspaces: Vec<Workspace>,
    #[cfg(target_os = "windows")] hardware_id: Option<String>,
    #[cfg(target_os = "windows")] device_path: Option<String>,
    #[cfg(target_os = "macos")] device_uuid: Option<String>,
  ) -> Self {
    let properties = NativeMonitorProperties::mock()
      .device_name(device_name)
      .bounds(bounds)
      .working_area(working_area)
      .dpi(dpi)
      .scale_factor(scale_factor)
      .maybe_hardware_id(hardware_id)
      .maybe_device_path(device_path)
      .call();

    let monitor = Self::new(native, properties);

    for workspace in workspaces {
      attach_container(&workspace.into(), &monitor.clone().into(), None)
        .expect("Failed to attach mock workspace.");
    }

    monitor
  }
}
```

Note: `bon` generates `maybe_<name>` setters for `Option<T>` parameters that take an `Option<T>` directly. On macOS, replace the two `maybe_hardware_id`/`maybe_device_path` lines with `.maybe_device_uuid(device_uuid)` under the matching `cfg`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p wm mock_monitor_carries_identity`
Expected: PASS

- [ ] **Step 5: Verify nothing else broke**

Run: `cargo test -p wm && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all tests pass, no warnings.

- [ ] **Step 6: Commit**

```bash
git add packages/wm/src/test_utils.rs
git commit -m "test: expose monitor identity fields on mock builders"
```

---

### Task 2: Layout store types and serde round-trip

Define the on-disk format. No file IO yet — this task is purely the data model and its serialisation.

**Files:**
- Create: `packages/wm/src/saved_layouts.rs`
- Modify: `packages/wm/src/main.rs` (add `mod saved_layouts;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `SavedMonitor { hardware_id: Option<String>, device_path: Option<String>, workspaces: Vec<String> }` (Windows; `device_uuid: Option<String>` on macOS).
  - `SavedLayout { saved_at: Option<u64>, monitors: Vec<SavedMonitor> }`
  - `SavedLayoutsFile { version: u32, layouts: HashMap<String, SavedLayout> }`
  - `const STORE_VERSION: u32 = 1;`

- [ ] **Step 1: Write the failing test**

Create `packages/wm/src/saved_layouts.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
  use super::{SavedLayout, SavedLayoutsFile, SavedMonitor, STORE_VERSION};

  /// Builds a representative store for round-trip testing.
  fn sample_file() -> SavedLayoutsFile {
    let mut file = SavedLayoutsFile::default();

    file.layouts.insert(
      "office".to_string(),
      SavedLayout {
        saved_at: Some(1_758_000_000),
        monitors: vec![
          SavedMonitor {
            hardware_id: Some("DELA26B".to_string()),
            device_path: Some("\\\\?\\DISPLAY#DELA26B#UID1".to_string()),
            workspaces: vec!["1".to_string(), "5".to_string()],
          },
          SavedMonitor {
            hardware_id: Some("AUO82B2".to_string()),
            device_path: None,
            workspaces: vec!["3".to_string()],
          },
        ],
      },
    );

    file
  }

  #[test]
  fn file_round_trips_through_yaml() {
    let original = sample_file();

    let yaml = serde_yaml::to_string(&original)
      .expect("Failed to serialize layout store.");

    let parsed: SavedLayoutsFile = serde_yaml::from_str(&yaml)
      .expect("Failed to deserialize layout store.");

    assert_eq!(parsed, original);
  }

  #[test]
  fn default_file_uses_current_version() {
    assert_eq!(SavedLayoutsFile::default().version, STORE_VERSION);
  }

  #[test]
  fn missing_optional_fields_default() {
    let yaml = "version: 1\nlayouts:\n  home:\n    monitors: []\n";

    let parsed: SavedLayoutsFile =
      serde_yaml::from_str(yaml).expect("Failed to parse minimal store.");

    let layout =
      parsed.layouts.get("home").expect("Missing 'home' layout.");

    assert_eq!(layout.saved_at, None);
    assert!(layout.monitors.is_empty());
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p wm saved_layouts`
Expected: FAIL to compile — `SavedLayoutsFile` not found.

- [ ] **Step 3: Implement the types**

Prepend to `packages/wm/src/saved_layouts.rs`:

```rust
//! Persistent store of named workspace-to-monitor layouts.
//!
//! Layouts are saved to `~/.glzr/glazewm/layouts.yaml` and restored when
//! the same set of displays is connected again. See
//! `docs/superpowers/specs/2026-09-22-workspace-layout-profiles-design.md`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Current schema version of the layout store.
///
/// A store with a higher version is loaded as empty and never written to,
/// so that an older build cannot destroy layouts it does not understand.
pub const STORE_VERSION: u32 = 1;

/// A monitor within a saved layout, identified by its stable hardware
/// properties rather than its position.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct SavedMonitor {
  /// Model identifier of the display (e.g. `DELA26B`).
  ///
  /// Portable between docks, but not unique when two displays share a
  /// model.
  #[cfg(target_os = "windows")]
  pub hardware_id: Option<String>,

  /// Full device path of the display.
  ///
  /// Unique, but embeds the adapter and port, so it can change when the
  /// same display is attached through a different dock.
  #[cfg(target_os = "windows")]
  pub device_path: Option<String>,

  /// Stable UUID of the display.
  #[cfg(target_os = "macos")]
  pub device_uuid: Option<String>,

  /// Names of the workspaces assigned to this monitor.
  ///
  /// Order is not meaningful; workspaces are always sorted by user config
  /// order when placed.
  pub workspaces: Vec<String>,
}

/// A named arrangement of workspaces across a set of monitors.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct SavedLayout {
  /// When the layout was saved, as Unix epoch seconds.
  ///
  /// Informational only, so a human editing the file can tell layouts
  /// apart. Nothing reads it and matching never consults it.
  pub saved_at: Option<u64>,

  /// Monitors in this layout, in no meaningful order.
  pub monitors: Vec<SavedMonitor>,
}

/// On-disk representation of the layout store.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct SavedLayoutsFile {
  /// Schema version of this file.
  pub version: u32,

  /// Layouts by user-chosen name.
  pub layouts: HashMap<String, SavedLayout>,
}

impl Default for SavedLayoutsFile {
  fn default() -> Self {
    Self {
      version: STORE_VERSION,
      layouts: HashMap::new(),
    }
  }
}
```

Then register the module in `packages/wm/src/main.rs`, keeping the existing alphabetical order of `mod` declarations:

```rust
mod pending_sync;
mod saved_layouts;
mod sys_tray;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p wm saved_layouts`
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/wm/src/saved_layouts.rs packages/wm/src/main.rs
git commit -m "feat: add layout store types"
```

---

### Task 3: Layout store file IO

Load and save the store, handling a missing file, corrupt YAML, and a future schema version.

**Files:**
- Modify: `packages/wm/src/saved_layouts.rs`

**Interfaces:**
- Consumes: `SavedLayoutsFile`, `SavedLayout`, `STORE_VERSION` from Task 2.
- Produces:
  - `SavedLayouts::load(path: PathBuf) -> Self` — never fails.
  - `SavedLayouts::save(&self) -> anyhow::Result<()>`
  - `SavedLayouts::upsert(&mut self, name: &str, layout: SavedLayout)`
  - `SavedLayouts::get(&self, name: &str) -> Option<&SavedLayout>`
  - `SavedLayouts::names(&self) -> Vec<String>`
  - `SavedLayouts::default_path() -> anyhow::Result<PathBuf>`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `packages/wm/src/saved_layouts.rs`:

```rust
  use std::{fs, path::PathBuf};

  use super::SavedLayouts;

  /// Creates an empty unique directory for a test to write into.
  fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir()
      .join(format!("glazewm-{label}-{}", uuid::Uuid::new_v4()));

    fs::create_dir_all(&dir).expect("Failed to create temp dir.");
    dir
  }

  #[test]
  fn missing_file_loads_as_empty() {
    let path = temp_dir("missing").join("layouts.yaml");
    let store = SavedLayouts::load(path);

    assert!(store.names().is_empty());
    assert!(!store.is_readonly());
  }

  #[test]
  fn saved_store_reloads_identically() {
    let path = temp_dir("roundtrip").join("layouts.yaml");

    let mut store = SavedLayouts::load(path.clone());
    store.upsert(
      "office",
      SavedLayout {
        saved_at: Some(42),
        monitors: vec![SavedMonitor {
          hardware_id: Some("DELA26B".to_string()),
          device_path: None,
          workspaces: vec!["1".to_string()],
        }],
      },
    );
    store.save().expect("Failed to save store.");

    let reloaded = SavedLayouts::load(path);

    assert_eq!(reloaded.names(), vec!["office".to_string()]);
    assert_eq!(
      reloaded.get("office").and_then(|l| l.monitors.first()),
      store.get("office").and_then(|l| l.monitors.first())
    );
  }

  #[test]
  fn corrupt_file_is_backed_up_and_store_is_empty() {
    let dir = temp_dir("corrupt");
    let path = dir.join("layouts.yaml");

    fs::write(&path, "this: is: not: valid: yaml:\n  - [")
      .expect("Failed to write corrupt file.");

    let store = SavedLayouts::load(path);

    assert!(store.names().is_empty());
    assert!(!store.is_readonly());

    let backups = fs::read_dir(&dir)
      .expect("Failed to read temp dir.")
      .filter_map(Result::ok)
      .filter(|entry| {
        entry.file_name().to_string_lossy().contains(".corrupt-")
      })
      .count();

    assert_eq!(backups, 1, "Expected exactly one backup file.");
  }

  #[test]
  fn future_version_is_readonly_and_refuses_to_save() {
    let path = temp_dir("future").join("layouts.yaml");

    fs::write(&path, "version: 999\nlayouts: {}\n")
      .expect("Failed to write future-version file.");

    let store = SavedLayouts::load(path);

    assert!(store.is_readonly());
    assert!(store.names().is_empty());
    assert!(store.save().is_err(), "Save must be refused.");
  }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p wm saved_layouts`
Expected: FAIL to compile — no `SavedLayouts` type.

- [ ] **Step 3: Implement the store**

Add to `packages/wm/src/saved_layouts.rs`, after the type definitions:

```rust
use std::{
  fs,
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context};

/// Runtime handle to the layout store.
#[derive(Clone, Debug)]
pub struct SavedLayouts {
  /// Parsed contents of the store.
  file: SavedLayoutsFile,

  /// Path the store was loaded from, and is saved back to.
  path: PathBuf,

  /// Whether writes are refused because the on-disk store has a newer
  /// schema version than this build understands.
  is_readonly: bool,
}

impl SavedLayouts {
  /// Loads the layout store from the given path.
  ///
  /// Never fails. A missing file yields an empty store, a corrupt file is
  /// renamed aside and yields an empty store, and a store with a newer
  /// schema version yields an empty read-only store.
  #[must_use]
  pub fn load(path: PathBuf) -> Self {
    let empty = Self {
      file: SavedLayoutsFile::default(),
      path: path.clone(),
      is_readonly: false,
    };

    let Ok(contents) = fs::read_to_string(&path) else {
      return empty;
    };

    match serde_yaml::from_str::<SavedLayoutsFile>(&contents) {
      Ok(file) if file.version > STORE_VERSION => {
        tracing::error!(
          "Layout store at {} has version {}, which is newer than the \
           supported version {}. Layouts will not be loaded or saved.",
          path.display(),
          file.version,
          STORE_VERSION
        );

        Self {
          is_readonly: true,
          ..empty
        }
      }
      Ok(file) => Self { file, ..empty },
      Err(err) => {
        tracing::error!(
          "Failed to parse layout store at {}: {}. Backing it up and \
           starting empty.",
          path.display(),
          err
        );

        Self::back_up_corrupt(&path);
        empty
      }
    }
  }

  /// Renames a corrupt store aside so it is never silently overwritten.
  fn back_up_corrupt(path: &Path) {
    let timestamp = Self::now_epoch_secs();
    let backup = path.with_extension(format!("yaml.corrupt-{timestamp}"));

    if let Err(err) = fs::rename(path, &backup) {
      tracing::error!(
        "Failed to back up corrupt layout store: {}. Leaving it in \
         place.",
        err
      );
    }
  }

  /// Default path of the layout store.
  pub fn default_path() -> anyhow::Result<PathBuf> {
    Ok(
      home::home_dir()
        .context("Unable to get home directory.")?
        .join(".glzr/glazewm/layouts.yaml"),
    )
  }

  /// Current time as Unix epoch seconds, or 0 if unavailable.
  fn now_epoch_secs() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_or(0, |duration| duration.as_secs())
  }

  /// Whether the store refuses writes.
  #[must_use]
  pub fn is_readonly(&self) -> bool {
    self.is_readonly
  }

  /// Names of all saved layouts, sorted alphabetically.
  #[must_use]
  pub fn names(&self) -> Vec<String> {
    let mut names =
      self.file.layouts.keys().cloned().collect::<Vec<_>>();

    names.sort();
    names
  }

  /// Gets a layout by name.
  #[must_use]
  pub fn get(&self, name: &str) -> Option<&SavedLayout> {
    self.file.layouts.get(name)
  }

  /// Inserts or replaces a layout, stamping it with the current time.
  pub fn upsert(&mut self, name: &str, mut layout: SavedLayout) {
    layout.saved_at = Some(Self::now_epoch_secs());
    self.file.layouts.insert(name.to_string(), layout);
  }

  /// Writes the store to disk.
  ///
  /// Writes to a temporary file and renames it into place, so an
  /// interrupted write cannot leave a corrupt store.
  ///
  /// # Errors
  ///
  /// Returns an error if the store is read-only, or if the file cannot be
  /// written.
  pub fn save(&self) -> anyhow::Result<()> {
    if self.is_readonly {
      bail!(
        "Refusing to overwrite a layout store with a newer schema \
         version."
      );
    }

    let parent = self
      .path
      .parent()
      .context("Invalid layout store path.")?;

    fs::create_dir_all(parent).with_context(|| {
      format!("Unable to create directory {}.", parent.display())
    })?;

    let serialized = serde_yaml::to_string(&self.file)
      .context("Failed to serialize layout store.")?;

    let temp_path = self.path.with_extension("yaml.tmp");

    fs::write(&temp_path, serialized).with_context(|| {
      format!("Unable to write to {}.", temp_path.display())
    })?;

    fs::rename(&temp_path, &self.path).with_context(|| {
      format!("Unable to replace {}.", self.path.display())
    })?;

    Ok(())
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p wm saved_layouts`
Expected: 7 tests PASS.

- [ ] **Step 5: Lint**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add packages/wm/src/saved_layouts.rs
git commit -m "feat: add layout store persistence"
```

---

### Task 4: Monitor identity matching

Match saved monitor entries to live monitors, and decide whether a layout exactly matches the current display set.

**Files:**
- Modify: `packages/wm/src/saved_layouts.rs`

**Interfaces:**
- Consumes: `SavedMonitor`, `SavedLayout`, `SavedLayouts` from Tasks 2-3; `Monitor` from `crate::models`.
- Produces:
  - `SavedMonitor::matches(&self, monitor: &Monitor, live: &[Monitor], saved: &[SavedMonitor]) -> bool`
  - `SavedLayout::resolve<'a>(&self, live: &'a [Monitor]) -> Vec<(&SavedMonitor, Monitor)>`
  - `SavedLayouts::exact_match(&self, live: &[Monitor]) -> Option<(String, &SavedLayout)>`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
  use crate::models::Monitor;

  /// Builds a live monitor with the given identity.
  fn live_monitor(
    hardware_id: Option<&str>,
    device_path: Option<&str>,
  ) -> Monitor {
    Monitor::mock()
      .maybe_hardware_id(hardware_id.map(str::to_string))
      .maybe_device_path(device_path.map(str::to_string))
      .call()
  }

  /// Builds a saved monitor entry with the given identity.
  fn saved_monitor(
    hardware_id: Option<&str>,
    device_path: Option<&str>,
  ) -> SavedMonitor {
    SavedMonitor {
      hardware_id: hardware_id.map(str::to_string),
      device_path: device_path.map(str::to_string),
      workspaces: vec![],
    }
  }

  #[test]
  fn matches_on_device_path() {
    let live = vec![live_monitor(Some("DELA26B"), Some("PATH-A"))];
    // Hardware id differs, so only the device path can match.
    let saved = vec![saved_monitor(Some("OTHER"), Some("PATH-A"))];

    assert!(saved[0].matches(&live[0], &live, &saved));
  }

  #[test]
  fn matches_on_unique_hardware_id_when_path_differs() {
    let live = vec![live_monitor(Some("DELA26B"), Some("NEW-DOCK"))];
    let saved = vec![saved_monitor(Some("DELA26B"), Some("OLD-DOCK"))];

    assert!(saved[0].matches(&live[0], &live, &saved));
  }

  #[test]
  fn does_not_match_ambiguous_hardware_id() {
    // Two identical displays, and device paths that no longer line up.
    let live = vec![
      live_monitor(Some("SAME"), Some("NEW-1")),
      live_monitor(Some("SAME"), Some("NEW-2")),
    ];
    let saved = vec![
      saved_monitor(Some("SAME"), Some("OLD-1")),
      saved_monitor(Some("SAME"), Some("OLD-2")),
    ];

    assert!(!saved[0].matches(&live[0], &live, &saved));
    assert!(!saved[1].matches(&live[1], &live, &saved));
  }

  #[test]
  fn exact_match_requires_every_monitor_present() {
    let mut store =
      SavedLayouts::load(temp_dir("exact").join("layouts.yaml"));

    store.upsert(
      "office",
      SavedLayout {
        saved_at: None,
        monitors: vec![
          saved_monitor(Some("A"), Some("PATH-A")),
          saved_monitor(Some("B"), Some("PATH-B")),
        ],
      },
    );

    let both = vec![
      live_monitor(Some("A"), Some("PATH-A")),
      live_monitor(Some("B"), Some("PATH-B")),
    ];
    assert!(store.exact_match(&both).is_some());

    // Subset: one display missing.
    let subset = vec![live_monitor(Some("A"), Some("PATH-A"))];
    assert!(store.exact_match(&subset).is_none());

    // Superset: an extra display present.
    let superset = vec![
      live_monitor(Some("A"), Some("PATH-A")),
      live_monitor(Some("B"), Some("PATH-B")),
      live_monitor(Some("C"), Some("PATH-C")),
    ];
    assert!(store.exact_match(&superset).is_none());
  }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p wm saved_layouts`
Expected: FAIL to compile — no method `matches` / `exact_match`.

- [ ] **Step 3: Implement matching**

Add to `packages/wm/src/saved_layouts.rs`. Import `crate::models::Monitor` at the top of the file:

```rust
impl SavedMonitor {
  /// Whether this saved entry identifies the given live monitor.
  ///
  /// Matching mirrors `find_matching_monitor` in the display settings
  /// handler, in priority order:
  ///
  /// 1. Device path equality, which is exact but changes between docks.
  /// 2. Hardware ID equality, but only when that ID is unambiguous on
  ///    both sides. Two identical displays share a hardware ID, so
  ///    matching on it would be a coin flip.
  ///
  /// `live` and `saved` are the full sets being matched, and are needed
  /// only to establish whether a hardware ID is unambiguous.
  #[must_use]
  pub fn matches(
    &self,
    monitor: &Monitor,
    live: &[Monitor],
    saved: &[SavedMonitor],
  ) -> bool {
    let properties = monitor.native_properties();

    #[cfg(target_os = "macos")]
    {
      return self
        .device_uuid
        .as_deref()
        .is_some_and(|uuid| uuid == properties.device_uuid);
    }

    #[cfg(target_os = "windows")]
    {
      let path_matches = self
        .device_path
        .as_deref()
        .zip(properties.device_path.as_deref())
        .is_some_and(|(saved_path, live_path)| saved_path == live_path);

      if path_matches {
        return true;
      }

      let Some(hardware_id) = self.hardware_id.as_deref() else {
        return false;
      };

      if properties.hardware_id.as_deref() != Some(hardware_id) {
        return false;
      }

      let live_count = live
        .iter()
        .filter(|other| {
          other.native_properties().hardware_id.as_deref()
            == Some(hardware_id)
        })
        .count();

      let saved_count = saved
        .iter()
        .filter(|other| other.hardware_id.as_deref() == Some(hardware_id))
        .count();

      live_count == 1 && saved_count == 1
    }
  }
}

impl SavedLayout {
  /// Pairs each saved monitor with the live monitor it identifies.
  ///
  /// Saved monitors that identify no live monitor are omitted, so the
  /// result may be shorter than `self.monitors`.
  #[must_use]
  pub fn resolve(&self, live: &[Monitor]) -> Vec<(&SavedMonitor, Monitor)> {
    self
      .monitors
      .iter()
      .filter_map(|saved| {
        live
          .iter()
          .find(|monitor| saved.matches(monitor, live, &self.monitors))
          .map(|monitor| (saved, monitor.clone()))
      })
      .collect()
  }
}

impl SavedLayouts {
  /// Finds the layout whose monitors exactly match the live display set.
  ///
  /// Exact means every saved monitor resolves to a live monitor, and no
  /// live monitor is left over. Automatic restore requires this, so that
  /// an unfamiliar display set never triggers a half-applied layout.
  ///
  /// Returns the layout's name alongside the layout.
  #[must_use]
  pub fn exact_match(
    &self,
    live: &[Monitor],
  ) -> Option<(String, &SavedLayout)> {
    self.file.layouts.iter().find_map(|(name, layout)| {
      let resolved = layout.resolve(live);

      let is_exact = resolved.len() == layout.monitors.len()
        && resolved.len() == live.len();

      is_exact.then(|| (name.clone(), layout))
    })
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p wm saved_layouts`
Expected: 11 tests PASS.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy --all-targets --all-features -- -D warnings
git add packages/wm/src/saved_layouts.rs
git commit -m "feat: add monitor identity matching for layouts"
```

---

### Task 5: Relocate `move_workspace_to_monitor`

Pure relocation so the primitive is not buried in `add_monitor.rs`. No behaviour change.

**Files:**
- Create: `packages/wm/src/commands/monitor/move_workspace_to_monitor.rs`
- Modify: `packages/wm/src/commands/monitor/add_monitor.rs:93-153` (remove function), `packages/wm/src/commands/monitor/mod.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `move_workspace_to_monitor(workspace: &Workspace, target_monitor: &Monitor, state: &mut WmState, config: &UserConfig) -> anyhow::Result<()>`, unchanged, re-exported from `commands::monitor` exactly as before.

- [ ] **Step 1: Move the function**

Cut `move_workspace_to_monitor` (currently `add_monitor.rs:93` to the end of that function) into the new file, adding the imports it needs:

```rust
use anyhow::Context;
use wm_common::WmEvent;

use crate::{
  commands::{
    container::move_container_within_tree,
    workspace::{activate_workspace, sort_workspaces},
  },
  models::{Monitor, Workspace},
  traits::{CommonGetters, PositionGetters, WindowGetters},
  user_config::UserConfig,
  wm_state::WmState,
};

/// Moves a workspace to the given monitor.
///
/// Activates a replacement workspace if the origin monitor would be left
/// with none, and re-sorts the target monitor's workspaces by config
/// order.
pub fn move_workspace_to_monitor(
  workspace: &Workspace,
  target_monitor: &Monitor,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  // ... body copied verbatim from `add_monitor.rs`.
}
```

Copy the body verbatim — do not alter it.

- [ ] **Step 2: Update module wiring**

In `packages/wm/src/commands/monitor/mod.rs`, keeping alphabetical order:

```rust
mod add_monitor;
mod focus_monitor;
mod move_workspace_to_monitor;
mod remove_monitor;
mod sort_monitors;
mod update_monitor;

pub use add_monitor::*;
pub use focus_monitor::*;
pub use move_workspace_to_monitor::*;
pub use remove_monitor::*;
pub use sort_monitors::*;
pub use update_monitor::*;
```

In `add_monitor.rs`, delete the moved function and add an import for it, since `move_bounded_workspaces_to_new_monitor` still calls it:

```rust
use crate::commands::monitor::move_workspace_to_monitor;
```

Then remove any imports in `add_monitor.rs` that are now unused — clippy will name them.

- [ ] **Step 3: Verify the build and full test suite**

Run: `cargo test -p wm && cargo clippy --all-targets --all-features -- -D warnings`
Expected: all existing tests PASS, no warnings. Nothing should change behaviourally.

- [ ] **Step 4: Commit**

```bash
git add packages/wm/src/commands/monitor/
git commit -m "refactor: move move_workspace_to_monitor to its own module"
```

---

### Task 6: Capture the current layout

**Files:**
- Create: `packages/wm/src/commands/monitor/save_workspace_layout.rs`
- Modify: `packages/wm/src/commands/monitor/mod.rs`

**Interfaces:**
- Consumes: `SavedLayout`, `SavedMonitor`, `SavedLayouts` (Tasks 2-3); `WmState::monitors()`.
- Produces:
  - `capture_layout(monitors: &[Monitor]) -> SavedLayout`
  - `save_workspace_layout(name: &str, state: &mut WmState) -> anyhow::Result<()>`

- [ ] **Step 1: Write the failing test**

Create `packages/wm/src/commands/monitor/save_workspace_layout.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
  use super::capture_layout;
  use crate::models::{Monitor, Workspace};

  #[test]
  fn captures_workspaces_per_monitor() {
    let left = Monitor::mock()
      .hardware_id(Some("DELA26B".to_string()))
      .device_path(Some("PATH-LEFT".to_string()))
      .workspaces(vec![
        Workspace::mock().name("1".to_string()).call(),
        Workspace::mock().name("5".to_string()).call(),
      ])
      .call();

    let right = Monitor::mock()
      .hardware_id(Some("AUO82B2".to_string()))
      .device_path(Some("PATH-RIGHT".to_string()))
      .workspaces(vec![Workspace::mock().name("3".to_string()).call()])
      .call();

    let layout = capture_layout(&[left, right]);

    assert_eq!(layout.monitors.len(), 2);
    assert_eq!(
      layout.monitors[0].hardware_id.as_deref(),
      Some("DELA26B")
    );
    assert_eq!(
      layout.monitors[0].workspaces,
      vec!["1".to_string(), "5".to_string()]
    );
    assert_eq!(
      layout.monitors[1].workspaces,
      vec!["3".to_string()]
    );
  }

  #[test]
  fn captures_monitor_with_no_workspaces() {
    let monitor = Monitor::mock()
      .hardware_id(Some("EMPTY".to_string()))
      .call();

    let layout = capture_layout(&[monitor]);

    assert_eq!(layout.monitors.len(), 1);
    assert!(layout.monitors[0].workspaces.is_empty());
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p wm captures_workspaces_per_monitor`
Expected: FAIL to compile — `capture_layout` not found.

- [ ] **Step 3: Implement**

Prepend to the same file:

```rust
use crate::{
  models::Monitor,
  saved_layouts::{SavedLayout, SavedMonitor},
  traits::CommonGetters,
  wm_state::WmState,
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

/// Saves the current workspace arrangement under the given name.
///
/// Replaces any existing layout with the same name, so re-saving after
/// rearranging is the intended way to update a layout.
///
/// # Errors
///
/// Returns an error if the layout store cannot be written.
pub fn save_workspace_layout(
  name: &str,
  state: &mut WmState,
) -> anyhow::Result<()> {
  let layout = capture_layout(&state.monitors());

  state.saved_layouts.upsert(name, layout);
  state.saved_layouts.save()?;

  tracing::info!("Saved workspace layout '{}'.", name);

  Ok(())
}
```

Add the module to `packages/wm/src/commands/monitor/mod.rs` in alphabetical order (`mod save_workspace_layout;` and `pub use save_workspace_layout::*;`).

Note: `state.saved_layouts` is added in Task 8. Until then this file will not compile on its own — that is expected, and Step 4 below only runs the `capture_layout` tests, which do not touch `WmState`. If the crate fails to build for that reason, temporarily comment out `save_workspace_layout` (not `capture_layout`), complete this task, and uncomment it during Task 8.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p wm captures_`
Expected: 2 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/wm/src/commands/monitor/
git commit -m "feat: capture current workspace layout"
```

---

### Task 7: Restore a layout

**Files:**
- Create: `packages/wm/src/commands/monitor/restore_workspace_layout.rs`
- Modify: `packages/wm/src/commands/monitor/mod.rs`

**Interfaces:**
- Consumes: `SavedLayout::resolve` (Task 4), `move_workspace_to_monitor` (Task 5).
- Produces: `apply_layout(layout: &SavedLayout, state: &mut WmState, config: &UserConfig) -> anyhow::Result<usize>` returning the number of workspaces moved.

- [ ] **Step 1: Write the failing test**

Create `packages/wm/src/commands/monitor/restore_workspace_layout.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
  use super::workspaces_to_move;
  use crate::{
    models::{Monitor, Workspace},
    saved_layouts::{SavedLayout, SavedMonitor},
  };

  /// Builds a saved monitor entry.
  fn saved(hardware_id: &str, workspaces: &[&str]) -> SavedMonitor {
    SavedMonitor {
      hardware_id: Some(hardware_id.to_string()),
      device_path: None,
      workspaces: workspaces.iter().map(|n| (*n).to_string()).collect(),
    }
  }

  #[test]
  fn moves_only_workspaces_that_are_elsewhere() {
    // Workspace 1 is already on the left monitor; workspace 3 is not.
    let left = Monitor::mock()
      .hardware_id(Some("LEFT".to_string()))
      .workspaces(vec![Workspace::mock().name("1".to_string()).call()])
      .call();

    let right = Monitor::mock()
      .hardware_id(Some("RIGHT".to_string()))
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

  #[test]
  fn ignores_workspaces_not_in_the_layout() {
    let left = Monitor::mock()
      .hardware_id(Some("LEFT".to_string()))
      .workspaces(vec![Workspace::mock().name("9".to_string()).call()])
      .call();

    let live = vec![left];

    let layout = SavedLayout {
      saved_at: None,
      monitors: vec![saved("LEFT", &[])],
    };

    assert!(workspaces_to_move(&layout, &live).is_empty());
  }

  #[test]
  fn skips_unmatched_monitors() {
    let left = Monitor::mock()
      .hardware_id(Some("LEFT".to_string()))
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p wm workspaces_to_move`
Expected: FAIL to compile — `workspaces_to_move` not found.

- [ ] **Step 3: Implement**

Prepend to the same file:

```rust
use anyhow::Context;

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
        (current.id() != target.id())
          .then(|| (workspace, target.clone()))
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

/// Restores a named layout, or the layout matching the current displays.
///
/// # Errors
///
/// Returns an error if the named layout does not exist, or if no layout
/// matches the current displays when no name is given.
pub fn restore_workspace_layout(
  name: Option<&str>,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let layout = match name {
    Some(name) => state
      .saved_layouts
      .get(name)
      .cloned()
      .with_context(|| {
        format!(
          "No layout named '{}'. Available layouts: {}.",
          name,
          state.saved_layouts.names().join(", ")
        )
      })?,
    None => state
      .saved_layouts
      .exact_match(&state.monitors())
      .map(|(_, layout)| layout.clone())
      .context("No saved layout matches the current displays.")?,
  };

  let count = apply_layout(&layout, state, config)?;

  tracing::info!("Restored workspace layout, moved {} workspaces.", count);

  Ok(())
}
```

Note: `SavedLayout` must derive `Clone` (it does, from Task 2) for the `.cloned()` calls above, which avoid holding a borrow of `state` across the mutable `apply_layout` call.

Add the module to `packages/wm/src/commands/monitor/mod.rs` in alphabetical order.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p wm workspaces_to_move`
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/wm/src/commands/monitor/
git commit -m "feat: apply a saved workspace layout"
```

---

### Task 8: Wire commands, state and startup

**Files:**
- Modify: `packages/wm-common/src/app_command.rs:251-261` (add variants)
- Modify: `packages/wm/src/wm_state.rs:31-105` (field + constructor)
- Modify: `packages/wm/src/wm.rs:57-72` (constructor) and the `InvokeCommand` match arm block ending at `:779`
- Modify: `packages/wm/src/main.rs:124-129`

**Interfaces:**
- Consumes: `save_workspace_layout` (Task 6), `restore_workspace_layout` (Task 7), `SavedLayouts` (Task 3).
- Produces: `InvokeCommand::WmSaveWorkspaceLayout { name: String }` and `InvokeCommand::WmRestoreWorkspaceLayout { name: Option<String> }`; `WmState.saved_layouts: SavedLayouts`.

- [ ] **Step 1: Write the failing test**

Add to `packages/wm-common/src/app_command.rs`, in a `#[cfg(test)]` module at the end of the file:

```rust
#[cfg(test)]
mod tests {
  use clap::Parser;

  use super::InvokeCommand;

  /// Parses a command string the way the user config does.
  fn parse(input: &str) -> InvokeCommand {
    let args = std::iter::once("").chain(input.split_whitespace());

    InvokeCommand::try_parse_from(args)
      .expect("Failed to parse command.")
  }

  #[test]
  fn parses_save_workspace_layout() {
    assert_eq!(
      parse("wm-save-workspace-layout --name office"),
      InvokeCommand::WmSaveWorkspaceLayout {
        name: "office".to_string()
      }
    );
  }

  #[test]
  fn parses_restore_workspace_layout_with_name() {
    assert_eq!(
      parse("wm-restore-workspace-layout --name home"),
      InvokeCommand::WmRestoreWorkspaceLayout {
        name: Some("home".to_string())
      }
    );
  }

  #[test]
  fn parses_restore_workspace_layout_without_name() {
    assert_eq!(
      parse("wm-restore-workspace-layout"),
      InvokeCommand::WmRestoreWorkspaceLayout { name: None }
    );
  }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p wm-common parses_`
Expected: FAIL to compile — variants not found.

- [ ] **Step 3: Add the command variants**

In `packages/wm-common/src/app_command.rs`, insert into `InvokeCommand` keeping the existing alphabetical ordering of the `Wm*` variants (after `WmRedraw`, before `WmReloadConfig`):

```rust
  WmRestoreWorkspaceLayout {
    #[clap(long)]
    name: Option<String>,
  },
  WmSaveWorkspaceLayout {
    #[clap(long)]
    name: String,
  },
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p wm-common parses_`
Expected: 3 tests PASS.

- [ ] **Step 5: Hold the store on `WmState`**

In `packages/wm/src/wm_state.rs`, add the field to the struct:

```rust
  /// Saved workspace layouts, keyed by user-chosen name.
  pub saved_layouts: SavedLayouts,
```

Add the import (`use crate::saved_layouts::SavedLayouts;`), extend the constructor signature and initialise it:

```rust
  pub fn new(
    dispatcher: Dispatcher,
    event_tx: mpsc::UnboundedSender<WmEvent>,
    exit_tx: mpsc::UnboundedSender<()>,
    saved_layouts: SavedLayouts,
  ) -> Self {
    Self {
      root_container: RootContainer::new(),
      dispatcher,
      pending_sync: PendingSync::default(),
      saved_layouts,
      // ... remaining fields unchanged.
    }
  }
```

In `packages/wm/src/wm.rs`, thread it through `WindowManager::new`:

```rust
  pub fn new(
    config: &mut UserConfig,
    dispatcher: Dispatcher,
    saved_layouts: SavedLayouts,
  ) -> anyhow::Result<Self> {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (exit_tx, exit_rx) = mpsc::unbounded_channel();

    let mut state =
      WmState::new(dispatcher, event_tx, exit_tx, saved_layouts);

    state.populate(config)?;

    Ok(Self {
      event_rx,
      exit_rx,
      state,
    })
  }
```

In `packages/wm/src/main.rs`, load the store just after the config and pass it in:

```rust
  // Parse and validate user config.
  let mut config = UserConfig::new(config_path)?;

  // Load saved workspace layouts. A failure here is non-fatal; the store
  // degrades to empty.
  let saved_layouts =
    SavedLayouts::load(SavedLayouts::default_path()?);

  // Add application icon to system tray.
  let mut tray = SystemTray::new(&config.path, dispatcher.clone())?;

  let mut wm =
    WindowManager::new(&mut config, dispatcher.clone(), saved_layouts)?;
```

Add `use crate::saved_layouts::SavedLayouts;` to `main.rs`.

- [ ] **Step 6: Dispatch the commands**

In `packages/wm/src/wm.rs`, add match arms alongside the other `Wm*` arms, and import the two commands from `crate::commands::monitor`:

```rust
      InvokeCommand::WmRestoreWorkspaceLayout { name } => {
        restore_workspace_layout(name.as_deref(), state, config)
      }
      InvokeCommand::WmSaveWorkspaceLayout { name } => {
        save_workspace_layout(name, state)
      }
```

If Task 6 left `save_workspace_layout` commented out, uncomment it now.

- [ ] **Step 7: Verify build, tests and lint**

Run: `cargo build -p wm && cargo test -p wm && cargo clippy --all-targets --all-features -- -D warnings`
Expected: builds, all tests pass, no warnings.

- [ ] **Step 8: Commit**

```bash
git add packages/wm-common/src/app_command.rs packages/wm/src/
git commit -m "feat: add save and restore workspace layout commands"
```

---

### Task 9: Automatic restore on display change

**Files:**
- Modify: `packages/wm-common/src/parsed_config.rs:72-99` (field) and `:101-124` (default)
- Modify: `packages/wm/src/events/handle_display_settings_changed.rs:16-120`
- Modify: `resources/assets/sample-config.yaml`

**Interfaces:**
- Consumes: `SavedLayouts::exact_match` (Task 4), `apply_layout` (Task 7).
- Produces: `general.restore_workspace_layout: bool`, defaulting to `true`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `packages/wm-common/src/parsed_config.rs` (create the module if absent):

```rust
#[cfg(test)]
mod tests {
  use super::GeneralConfig;

  #[test]
  fn restore_workspace_layout_defaults_to_true() {
    assert!(GeneralConfig::default().restore_workspace_layout);
  }

  #[test]
  fn restore_workspace_layout_can_be_disabled() {
    let yaml = "restore_workspace_layout: false\n";

    let config: GeneralConfig =
      serde_yaml::from_str(yaml).expect("Failed to parse config.");

    assert!(!config.restore_workspace_layout);
  }
}
```

Note: `serde_yaml` is not currently a dependency of `wm-common`. If it is absent, drop the second test and keep only the default test, which needs no extra dependency. Do **not** add a dependency for a test.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p wm-common restore_workspace_layout`
Expected: FAIL to compile — no field `restore_workspace_layout`.

- [ ] **Step 3: Add the config field**

In `packages/wm-common/src/parsed_config.rs`, add to `GeneralConfig`:

```rust
  /// Whether to automatically restore a saved workspace layout when the
  /// connected displays exactly match one.
  pub restore_workspace_layout: bool,
```

And to its `Default` impl:

```rust
      restore_workspace_layout: true,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p wm-common restore_workspace_layout`
Expected: PASS

- [ ] **Step 5: Add the restore hook**

In `packages/wm/src/events/handle_display_settings_changed.rs`, record whether the monitor set changed, then attempt a restore at the end.

Before the removal loop, capture whether anything was added:

```rust
  let has_added_monitors = !new_monitors.is_empty();
```

Track removals by replacing the removal loop with one that records them:

```rust
  let mut has_removed_monitors = false;

  for monitor in pending_monitors {
    if state.monitors().len() > 1 {
      remove_monitor(monitor, state, config)?;
      has_removed_monitors = true;
    }
  }
```

Then, immediately after the existing `move_bounded_workspaces_to_new_monitor` loop and before the window DPI loop:

```rust
  // Restore a saved layout when the set of displays has changed and
  // exactly matches one. Requiring an exact match also debounces the
  // burst of events emitted while docking, since no layout can match
  // until every display has arrived.
  if config.value.general.restore_workspace_layout
    && (has_added_monitors || has_removed_monitors)
  {
    let matched = state
      .saved_layouts
      .exact_match(&state.monitors())
      .map(|(name, layout)| (name, layout.clone()));

    if let Some((name, layout)) = matched {
      match apply_layout(&layout, state, config) {
        Ok(count) => tracing::info!(
          "Restored workspace layout '{}', moved {} workspaces.",
          name,
          count
        ),
        Err(err) => {
          tracing::error!("Failed to restore workspace layout: {}", err);
        }
      }
    }
  }
```

Add `apply_layout` to the existing `crate::commands::monitor` import list at the top of the file.

Note the restore failure is logged rather than propagated: a display change must never fail because a layout could not be applied.

- [ ] **Step 6: Document the option**

In `resources/assets/sample-config.yaml`, under `general:`, add:

```yaml
  # Whether to automatically restore a saved workspace layout when the
  # connected displays exactly match one. Layouts are saved with the
  # `wm-save-workspace-layout` command.
  restore_workspace_layout: true
```

- [ ] **Step 7: Verify build, tests and lint**

Run: `cargo test --workspace && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: all pass, no warnings, formatting clean.

- [ ] **Step 8: Commit**

```bash
git add packages/wm-common/src/parsed_config.rs packages/wm/src/events/ resources/assets/sample-config.yaml
git commit -m "feat: restore workspace layout on display change"
```

---

### Task 10: End-to-end verification

No code. This task proves the feature works against the real window manager.

**Files:**
- None modified.

**Interfaces:**
- Consumes: everything.
- Produces: a verified build.

- [ ] **Step 1: Stop the installed instance**

The installed GlazeWM and a local build fight over the single-instance lock and IPC port 6123.

```bash
powershell.exe -Command "Stop-Process -Name glazewm -Force -ErrorAction SilentlyContinue"
```

- [ ] **Step 2: Build and run the local WM**

```bash
cargo build --workspace
cargo run -- start -v
```

Expected: starts without error, tray icon appears.

- [ ] **Step 3: Save a layout**

Arrange workspaces across monitors as desired, then:

```bash
./target/debug/glazewm-cli command wm-save-workspace-layout --name office
cat ~/.glzr/glazewm/layouts.yaml
```

Expected: `layouts.yaml` exists, contains an `office` layout with one entry per monitor, each listing that monitor's workspace names and its `hardware_id`/`device_path`.

- [ ] **Step 4: Simulate an undock**

**Ask the user before running this — it blanks and rearranges their screens.**

```bash
powershell.exe -Command "DisplaySwitch.exe /internal"
```

Expected: workspaces consolidate onto the laptop display, as they do today.

- [ ] **Step 5: Simulate a redock**

```bash
powershell.exe -Command "DisplaySwitch.exe /extend"
```

Expected: once all displays are back, workspaces return to the monitors recorded in the `office` layout. Check the log output for `Restored workspace layout 'office', moved N workspaces.`

- [ ] **Step 6: Verify the manual command and error paths**

```bash
./target/debug/glazewm-cli command wm-restore-workspace-layout --name office
./target/debug/glazewm-cli command wm-restore-workspace-layout --name nope
```

Expected: the first succeeds; the second fails with a message naming the available layouts.

- [ ] **Step 7: Check for errors**

```bash
tail -20 ~/.glzr/glazewm/errors.log
```

Expected: no new errors relating to layouts.

- [ ] **Step 8: Commit any fixes, then report**

Report to the user: whether auto-restore fired, how many workspaces moved, and the contents of `layouts.yaml`. The genuine test remains their next real commute between desks, saving a second layout named `home` on arrival.

---

## Self-review

**Spec coverage.** Storage format → Task 2-3. Module location → Task 2. Identity matching → Task 4. Commands → Task 6-8. Restore flow → Task 7. Auto-restore trigger and guards → Task 9. Config toggle → Task 9. `move_workspace_to_monitor` relocation → Task 5. Failure handling table: missing file, corrupt YAML, future version, write failure → Task 3; inactive workspace and unmatched monitor → Task 7; unknown layout name → Task 7. Testing list → Tasks 2, 3, 4, 6, 7, 8, 9. Manual verification → Task 10.

**Known deviation from the spec:** `saved_at` is Unix epoch seconds, not an RFC3339 string, to avoid adding a date dependency for an informational field. Recorded in Global Constraints.

**Type consistency.** `SavedMonitor`, `SavedLayout`, `SavedLayoutsFile`, `SavedLayouts` are used consistently from Task 2 onward. `capture_layout` (Task 6) produces `SavedLayout`; `workspaces_to_move` and `apply_layout` (Task 7) consume it; `exact_match` (Task 4) returns `Option<(String, &SavedLayout)>` and both callers (Task 7, Task 9) destructure that tuple. `move_workspace_to_monitor` keeps its exact signature through the Task 5 relocation.
