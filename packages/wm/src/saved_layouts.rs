//! Persistent store of named workspace-to-monitor layouts.
//!
//! Layouts are saved to `~/.glzr/glazewm/layouts.yaml` and restored when
//! the same set of displays is connected again. See
//! `docs/superpowers/specs/2026-09-22-workspace-layout-profiles-design.
//! md`.

// Temporary: these types are consumed from Task 8 onward, when this
// allow is removed.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{models::Monitor, traits::CommonGetters};

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
      path,
      is_readonly: false,
    };

    let Ok(contents) = fs::read_to_string(&empty.path) else {
      return empty;
    };

    match serde_yaml::from_str::<SavedLayoutsFile>(&contents) {
      Ok(file) if file.version > STORE_VERSION => {
        tracing::error!(
          "Layout store at {} has version {}, which is newer than the \
           supported version {}. Layouts will not be loaded or saved.",
          empty.path.display(),
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
          empty.path.display(),
          err
        );

        Self::back_up_corrupt(&empty.path);
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
  ///
  /// # Errors
  ///
  /// Returns an error if the home directory cannot be determined.
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
    let mut names = self.file.layouts.keys().cloned().collect::<Vec<_>>();

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

    let parent =
      self.path.parent().context("Invalid layout store path.")?;

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

  /// Finds the layout whose monitors exactly match the live display set.
  ///
  /// Exact means every saved monitor resolves to a live monitor, and no
  /// live monitor is left over. Automatic restore requires this, so that
  /// an unfamiliar display set never triggers a half-applied layout.
  ///
  /// If more than one layout matches, the one that sorts first by name
  /// is chosen, and the ambiguity is logged, so the choice is
  /// deterministic and diagnosable rather than dependent on hash order.
  ///
  /// Returns the layout's name alongside the layout.
  #[must_use]
  pub fn exact_match(
    &self,
    live: &[Monitor],
  ) -> Option<(String, &SavedLayout)> {
    let mut candidates = self
      .file
      .layouts
      .iter()
      .filter(|(_, layout)| {
        let resolved = layout.resolve(live);

        resolved.len() == layout.monitors.len()
          && resolved.len() == live.len()
      })
      .collect::<Vec<_>>();

    candidates.sort_unstable_by_key(|(name, _)| name.as_str());

    if candidates.len() > 1 {
      let names = candidates
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();

      tracing::warn!(
        "Multiple saved layouts match the current display set: {}. \
         Choosing '{}'.",
        names.join(", "),
        names[0]
      );
    }

    candidates
      .into_iter()
      .next()
      .map(|(name, layout)| (name.clone(), layout))
  }
}

impl SavedMonitor {
  /// Whether this saved entry identifies the given live monitor.
  ///
  /// Matching mirrors `find_matching_monitor` in the display settings
  /// handler, in priority order:
  ///
  /// 1. Device path equality, which is exact but changes between docks.
  /// 2. Hardware ID equality, but only when that ID is unambiguous on both
  ///    sides. Two identical displays share a hardware ID, so matching on
  ///    it would be a coin flip.
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
      // `live` and `saved` are only needed to disambiguate hardware IDs,
      // which is a Windows-only concern.
      let _ = (live, saved);

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
  /// result may be shorter than `self.monitors`. Each live monitor is
  /// claimed by at most one saved entry, in `self.monitors` order, so
  /// two saved entries that both identify the same live monitor (e.g.
  /// a duplicated entry in a hand-edited store) never both resolve to
  /// it.
  #[must_use]
  pub fn resolve(
    &self,
    live: &[Monitor],
  ) -> Vec<(&SavedMonitor, Monitor)> {
    let mut claimed = HashSet::new();

    self
      .monitors
      .iter()
      .filter_map(|saved| {
        live
          .iter()
          .find(|monitor| {
            !claimed.contains(&monitor.id())
              && saved.matches(monitor, live, &self.monitors)
          })
          .map(|monitor| {
            claimed.insert(monitor.id());
            (saved, monitor.clone())
          })
      })
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use std::{fs, path::PathBuf};

  // `SavedLayout` and `SavedMonitor` are only named by tests
  // exercising `SavedMonitor`'s identity fields, which are
  // Windows-only.
  #[cfg(target_os = "windows")]
  use super::{SavedLayout, SavedMonitor};
  use super::{SavedLayouts, SavedLayoutsFile, STORE_VERSION};
  #[cfg(target_os = "windows")]
  use crate::models::Monitor;

  /// Creates an empty unique directory for a test to write into.
  fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir()
      .join(format!("glazewm-{label}-{}", uuid::Uuid::new_v4()));

    fs::create_dir_all(&dir).expect("Failed to create temp dir.");
    dir
  }

  /// Builds a representative store for round-trip testing.
  ///
  /// Windows-only: `SavedMonitor`'s identity fields are Windows-only.
  #[cfg(target_os = "windows")]
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

  #[cfg(target_os = "windows")]
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
    assert_eq!(layout.monitors, vec![]);
  }

  #[test]
  fn missing_file_loads_as_empty() {
    let path = temp_dir("missing").join("layouts.yaml");
    let store = SavedLayouts::load(path);

    assert_eq!(store.names(), Vec::<String>::new());
    assert!(!store.is_readonly());
  }

  // Windows-only: constructs `SavedMonitor` literals with the
  // Windows-only identity fields.
  #[cfg(target_os = "windows")]
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

    assert_eq!(store.names(), Vec::<String>::new());
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
    assert_eq!(store.names(), Vec::<String>::new());
    assert!(store.save().is_err(), "Save must be refused.");
  }

  /// Builds a live monitor with the given identity.
  #[cfg(target_os = "windows")]
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
  #[cfg(target_os = "windows")]
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

  #[cfg(target_os = "windows")]
  #[test]
  fn matches_on_device_path() {
    let live = vec![live_monitor(Some("DELA26B"), Some("PATH-A"))];
    // Hardware id differs, so only the device path can match.
    let saved = vec![saved_monitor(Some("OTHER"), Some("PATH-A"))];

    assert!(saved[0].matches(&live[0], &live, &saved));
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn matches_on_unique_hardware_id_when_path_differs() {
    let live = vec![live_monitor(Some("DELA26B"), Some("NEW-DOCK"))];
    let saved = vec![saved_monitor(Some("DELA26B"), Some("OLD-DOCK"))];

    assert!(saved[0].matches(&live[0], &live, &saved));
  }

  #[cfg(target_os = "windows")]
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

  #[cfg(target_os = "windows")]
  #[test]
  fn does_not_match_ambiguous_saved_hardware_id() {
    // A single, unambiguous live display, but two saved entries share
    // its hardware id and neither's device path lines up. A guard
    // that only checked the live side would wrongly match here.
    let live = vec![live_monitor(Some("SAME"), Some("NEW-1"))];
    let saved = vec![
      saved_monitor(Some("SAME"), Some("OLD-1")),
      saved_monitor(Some("SAME"), Some("OLD-2")),
    ];

    assert!(!saved[0].matches(&live[0], &live, &saved));
    assert!(!saved[1].matches(&live[0], &live, &saved));
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn resolve_claims_each_live_monitor_at_most_once() {
    let live = vec![
      live_monitor(Some("A"), Some("PATH-A")),
      live_monitor(Some("B"), Some("PATH-B")),
    ];

    // Two saved entries identifying the same live monitor, as a
    // hand-edited layouts.yaml could produce. Neither identifies the
    // second live monitor.
    let layout = SavedLayout {
      saved_at: None,
      monitors: vec![
        saved_monitor(Some("A"), Some("PATH-A")),
        saved_monitor(Some("A"), Some("PATH-A")),
      ],
    };

    let resolved = layout.resolve(&live);
    assert_eq!(
      resolved.len(),
      1,
      "Only one saved entry should claim the live monitor; the other \
       has nothing left to resolve to."
    );

    let mut store =
      SavedLayouts::load(temp_dir("dedup").join("layouts.yaml"));
    store.upsert("dup", layout);

    assert!(
      store.exact_match(&live).is_none(),
      "The second live monitor is never claimed, so this must not be \
       reported as an exact match."
    );
  }

  #[cfg(target_os = "windows")]
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
}
