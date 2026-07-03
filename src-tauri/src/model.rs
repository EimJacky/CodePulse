use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexStatus {
    Offline,
    Idle,
    Thinking,
    Executing,
    WaitingApproval,
    Completed,
    Failed,
}

impl CodexStatus {
    pub fn priority(self) -> u8 {
        match self {
            Self::Offline => 0,
            Self::Idle => 1,
            Self::Completed => 2,
            Self::Thinking => 3,
            Self::Executing => 4,
            Self::WaitingApproval => 5,
            Self::Failed => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThreadConfidence {
    Normal,
    Limited,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadState {
    pub id: String,
    pub title: String,
    pub status: CodexStatus,
    pub status_since: i64,
    pub last_changed_at: i64,
    pub confidence: ThreadConfidence,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub level: HealthLevel,
    pub message: String,
    pub last_successful_read: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HealthLevel {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub aggregate: CodexStatus,
    pub threads: Vec<ThreadState>,
    pub health: ProviderHealth,
    pub observed_at: i64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WindowAnchor {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowPlacement {
    pub monitor_name: Option<String>,
    pub anchor: WindowAnchor,
    pub offset_x: i32,
    pub offset_y: i32,
    pub scale_factor: f64,
}

impl Default for WindowPlacement {
    fn default() -> Self {
        Self {
            monitor_name: None,
            anchor: WindowAnchor::TopRight,
            offset_x: 24,
            offset_y: 24,
            scale_factor: 1.0,
        }
    }
}

fn default_opacity() -> f64 {
    0.94
}
fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_opacity")]
    pub opacity: f64,
    #[serde(default = "yes")]
    pub animations: bool,
    #[serde(default = "yes")]
    pub always_on_top: bool,
    #[serde(default = "yes")]
    pub edge_snap: bool,
    pub launch_at_startup: bool,
    #[serde(default = "yes")]
    pub remember_position: bool,
    #[serde(default = "yes")]
    pub double_click_focus: bool,
    #[serde(default = "yes")]
    pub show_provider_warnings: bool,
    pub placement: Option<WindowPlacement>,
    // 0.1.x migration fields. They are read once and omitted on the next save.
    #[serde(default, skip_serializing)]
    pub window_x: Option<i32>,
    #[serde(default, skip_serializing)]
    pub window_y: Option<i32>,
    #[serde(default, skip_serializing)]
    pub paused: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            opacity: default_opacity(),
            animations: true,
            always_on_top: true,
            edge_snap: true,
            launch_at_startup: false,
            remember_position: true,
            double_click_focus: true,
            show_provider_warnings: true,
            placement: None,
            window_x: None,
            window_y: None,
            paused: false,
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.opacity = self.opacity.clamp(0.72, 1.0);
        if self.placement.is_none() {
            if let (Some(x), Some(y)) = (self.window_x, self.window_y) {
                self.placement = Some(WindowPlacement {
                    monitor_name: None,
                    anchor: WindowAnchor::TopLeft,
                    offset_x: x,
                    offset_y: y,
                    scale_factor: 1.0,
                });
            }
        }
        self.window_x = None;
        self.window_y = None;
        self.paused = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_coordinates_and_preserves_values() {
        let json = r#"{"opacity":0.83,"animations":false,"alwaysOnTop":true,"edgeSnap":true,"launchAtStartup":false,"paused":true,"windowX":120,"windowY":80}"#;
        let settings: AppSettings = serde_json::from_str(json).unwrap();
        let migrated = settings.normalized();
        assert_eq!(migrated.opacity, 0.83);
        assert!(!migrated.animations);
        assert!(!migrated.paused);
        assert_eq!(migrated.placement.unwrap().offset_x, 120);
    }

    #[test]
    fn priority_matches_product_contract() {
        assert!(CodexStatus::Failed.priority() > CodexStatus::WaitingApproval.priority());
        assert!(CodexStatus::WaitingApproval.priority() > CodexStatus::Executing.priority());
        assert!(CodexStatus::Executing.priority() > CodexStatus::Thinking.priority());
    }
}
