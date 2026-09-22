//! Persistent store of named workspace-to-monitor layouts.
//!
//! Layouts are saved to `~/.glzr/glazewm/layouts.yaml` and restored when
//! the same set of displays is connected again. See
//! `docs/superpowers/specs/2026-09-22-workspace-layout-profiles-design.
//! md`.

// Temporary: these types are consumed from Task 8 onward, when this
// allow is removed.
#![allow(dead_code)]

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

#[cfg(test)]
mod tests {
  use super::{
    SavedLayout, SavedLayoutsFile, SavedMonitor, STORE_VERSION,
  };

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
    assert_eq!(layout.monitors, vec![]);
  }
}
