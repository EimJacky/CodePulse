use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexStatus { Offline, Idle, Thinking, Executing, WaitingApproval, Completed, Failed }

impl CodexStatus {
    pub fn priority(self) -> u8 { match self { Self::Offline => 0, Self::Idle => 1, Self::Completed => 2, Self::Thinking => 3, Self::Executing => 4, Self::WaitingApproval => 5, Self::Failed => 6 } }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadState { pub id: String, pub title: String, pub status: CodexStatus, pub since: i64, pub last_changed_at: i64 }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth { pub level: HealthLevel, pub message: String, pub last_successful_read: Option<i64> }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HealthLevel { Healthy, Degraded, Unavailable }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot { pub aggregate: CodexStatus, pub threads: Vec<ThreadState>, pub health: ProviderHealth, pub observed_at: i64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub opacity: f64,
    pub animations: bool,
    pub always_on_top: bool,
    pub edge_snap: bool,
    pub launch_at_startup: bool,
    pub paused: bool,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
}

impl Default for AppSettings {
    fn default() -> Self { Self { opacity: 0.96, animations: true, always_on_top: true, edge_snap: true, launch_at_startup: false, paused: false, window_x: None, window_y: None } }
}
