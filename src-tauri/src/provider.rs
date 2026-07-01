use crate::model::{CodexStatus, HealthLevel, ProviderHealth, StatusSnapshot, ThreadState};
use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{fs, path::{Path, PathBuf}, time::{Duration, SystemTime}};
use sysinfo::{ProcessesToUpdate, System};

pub trait CodexStateProvider: Send {
    fn snapshot(&mut self) -> StatusSnapshot;
}

pub struct LocalCodexProvider { home: PathBuf, system: System }

impl LocalCodexProvider {
    pub fn new(home: PathBuf) -> Self { Self { home, system: System::new() } }

    fn codex_running(&mut self) -> bool {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.system.processes().values().any(|p| {
            let name = p.name().to_string_lossy().to_ascii_lowercase();
            name.contains("codex")
        })
    }

    fn read_threads(&self, now: i64) -> Result<Vec<ThreadState>, String> {
        let db = self.home.join("state_5.sqlite");
        let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(|e| e.to_string())?;
        conn.busy_timeout(Duration::from_millis(100)).map_err(|e| e.to_string())?;
        let mut statement = conn.prepare("SELECT id, COALESCE(title, '未命名任务'), rollout_path, COALESCE(updated_at_ms, updated_at * 1000, 0) FROM threads WHERE archived = 0 ORDER BY COALESCE(updated_at_ms, updated_at * 1000) DESC LIMIT 8").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, i64>(3)?))).map_err(|e| e.to_string())?;
        let mut threads = Vec::new();
        for row in rows.flatten() {
            let age = now.saturating_sub(row.3);
            if age > 15 * 60_000 { continue; }
            let status = row.2.as_deref().map(Path::new).and_then(|path| infer_rollout_status(path, now)).unwrap_or_else(|| if age < 12_000 { CodexStatus::Thinking } else { CodexStatus::Idle });
            if status != CodexStatus::Idle || age < 120_000 {
                threads.push(ThreadState { id: row.0, title: row.1, status, since: row.3, last_changed_at: row.3 });
            }
        }
        Ok(threads)
    }
}

impl CodexStateProvider for LocalCodexProvider {
    fn snapshot(&mut self) -> StatusSnapshot {
        let now = Utc::now().timestamp_millis();
        if !self.codex_running() {
            return StatusSnapshot { aggregate: CodexStatus::Offline, threads: vec![], health: ProviderHealth { level: HealthLevel::Unavailable, message: "Codex 未运行".into(), last_successful_read: None }, observed_at: now };
        }
        match self.read_threads(now) {
            Ok(threads) => {
                let aggregate = threads.iter().map(|t| t.status).max_by_key(|s| s.priority()).unwrap_or(CodexStatus::Idle);
                StatusSnapshot { aggregate, threads, health: ProviderHealth { level: HealthLevel::Healthy, message: "状态探测正常".into(), last_successful_read: Some(now) }, observed_at: now }
            }
            Err(error) => StatusSnapshot { aggregate: CodexStatus::Idle, threads: vec![], health: ProviderHealth { level: HealthLevel::Degraded, message: format!("已降级为进程探测：{error}"), last_successful_read: None }, observed_at: now }
        }
    }
}

fn infer_rollout_status(path: &Path, now: i64) -> Option<CodexStatus> {
    let modified = fs::metadata(path).ok()?.modified().ok()?.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_millis() as i64;
    let age = now.saturating_sub(modified);
    let text = fs::read_to_string(path).ok()?;
    let mut latest = None;
    for line in text.lines().rev().take(80) {
        let value: Value = serde_json::from_str(line).ok()?;
        let kind = value.get("payload").and_then(|p| p.get("type")).and_then(Value::as_str).unwrap_or_default();
        let status = match kind {
            "task_complete" => Some(if age < 8_000 { CodexStatus::Completed } else { CodexStatus::Idle }),
            "task_started" | "reasoning" => Some(CodexStatus::Thinking),
            "function_call" | "custom_tool_call" | "patch_apply_begin" => Some(CodexStatus::Executing),
            "turn_aborted" | "error" => Some(CodexStatus::Failed),
            "approval_request" | "request_user_input" => Some(CodexStatus::WaitingApproval),
            _ => None,
        };
        if status.is_some() { latest = status; break; }
    }
    latest.map(|status| if age > 45_000 && matches!(status, CodexStatus::Thinking | CodexStatus::Executing) { CodexStatus::Idle } else { status })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn maps_rollout_events() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\"}}}}").unwrap();
        assert_eq!(infer_rollout_status(file.path(), Utc::now().timestamp_millis()), Some(CodexStatus::Thinking));
    }
    #[test]
    fn priority_is_stable() { assert!(CodexStatus::Failed.priority() > CodexStatus::WaitingApproval.priority()); }
}
