use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BindingModeConfig, ContainerDto, TilingDirection, WmEvent};

pub const DEFAULT_IPC_PORT: u32 = 6123;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "messageType", rename_all = "snake_case")]
pub enum ServerMessage {
  ClientResponse(ClientResponseMessage),
  EventSubscription(EventSubscriptionMessage),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientResponseMessage {
  pub client_message: String,
  pub data: Option<ClientResponseData>,
  pub error: Option<String>,
  pub success: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ClientResponseData {
  AppMetadata(AppMetadataData),
  BindingModes(BindingModesData),
  Command(CommandData),
  EventSubscribe(EventSubscribeData),
  EventUnsubscribe,
  Focused(FocusedData),
  Monitors(MonitorsData),
  TilingDirection(TilingDirectionData),
  Windows(WindowsData),
  Workspaces(WorkspacesData),
  // Must follow every other object-shaped variant; see
  // `WorkspaceLayoutData`.
  WorkspaceLayout(WorkspaceLayoutData),
  Paused(bool),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadataData {
  pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingModesData {
  pub binding_modes: Vec<BindingModeConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandData {
  pub subject_container_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSubscribeData {
  pub subscription_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusedData {
  pub focused: ContainerDto,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorsData {
  pub monitors: Vec<ContainerDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TilingDirectionData {
  pub tiling_direction: TilingDirection,
  pub direction_container: ContainerDto,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsData {
  pub windows: Vec<ContainerDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesData {
  pub workspaces: Vec<ContainerDto>,
}

/// The saved workspace layout matching the connected displays.
///
/// Rejects unknown fields because `ClientResponseData` is untagged: a
/// struct whose only field is optional would otherwise deserialize from
/// any JSON object and swallow responses meant for other variants.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceLayoutData {
  /// Name of the matching layout, or `None` if no saved layout matches.
  pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSubscriptionMessage {
  pub data: Option<WmEvent>,
  pub error: Option<String>,
  pub subscription_id: Uuid,
  pub success: bool,
}

#[cfg(test)]
mod tests {
  use super::{ClientResponseData, WorkspaceLayoutData};

  /// Round-trips response data through JSON, as the IPC client does.
  fn round_trip(data: &ClientResponseData) -> ClientResponseData {
    let json =
      serde_json::to_string(data).expect("Failed to serialize data.");

    serde_json::from_str(&json).expect("Failed to deserialize data.")
  }

  #[test]
  fn workspace_layout_round_trips() {
    for name in [Some("home".to_string()), None] {
      let data =
        ClientResponseData::WorkspaceLayout(WorkspaceLayoutData {
          name: name.clone(),
        });

      assert!(
        matches!(
          round_trip(&data),
          ClientResponseData::WorkspaceLayout(WorkspaceLayoutData {
            name: parsed,
          }) if parsed == name
        ),
        "Expected a workspace layout response for {name:?}."
      );
    }
  }

  #[test]
  fn workspace_layout_does_not_swallow_other_objects() {
    let parsed: ClientResponseData =
      serde_json::from_str(r#"{"unrelated":true}"#)
        .unwrap_or(ClientResponseData::EventUnsubscribe);

    assert!(
      !matches!(parsed, ClientResponseData::WorkspaceLayout(_)),
      "An unrecognised object must not parse as a workspace layout."
    );
  }
}
