use crate::model::{
    CodexStatus, HealthLevel, ProviderHealth, StatusSnapshot, ThreadConfidence, ThreadState,
};
use chrono::{DateTime, Utc};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
};
use sysinfo::{ProcessesToUpdate, System};
use tokio::sync::mpsc::{self, UnboundedReceiver};

const INITIAL_TAIL_BYTES: u64 = 256 * 1024;
const COMPLETED_HOLD_MS: i64 = 8_000;
const FAILED_HOLD_MS: i64 = 15_000;

pub trait CodexStateProvider: Send {
    fn snapshot(&mut self) -> StatusSnapshot;
}

#[derive(Clone, Debug)]
struct ReducedEvent {
    status: CodexStatus,
    at: i64,
}

#[derive(Default)]
struct RolloutCache {
    offset: u64,
    last: Option<ReducedEvent>,
    malformed_lines: usize,
}

pub struct LocalCodexProvider {
    home: PathBuf,
    system: System,
    rollout_cache: HashMap<PathBuf, RolloutCache>,
    _watcher: Option<RecommendedWatcher>,
    change_rx: Option<UnboundedReceiver<()>>,
}

impl LocalCodexProvider {
    pub fn new(home: PathBuf) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                if let Ok(event) = result {
                    let relevant = event.paths.iter().any(|path| {
                        let name = path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default();
                        path.extension().and_then(|ext| ext.to_str()) == Some("jsonl")
                            || (name.starts_with("state_")
                                && (name.contains(".sqlite") || name.ends_with("-wal")))
                    });
                    if relevant {
                        let _ = tx.send(());
                    }
                }
            })
            .ok();

        if let Some(inner) = watcher.as_mut() {
            let sessions = home.join("sessions");
            if sessions.exists() {
                let _ = inner.watch(&sessions, RecursiveMode::Recursive);
            }
            if home.exists() {
                let _ = inner.watch(&home, RecursiveMode::NonRecursive);
            }
        }

        Self {
            home,
            system: System::new(),
            rollout_cache: HashMap::new(),
            _watcher: watcher,
            change_rx: Some(rx),
        }
    }

    pub fn take_change_receiver(&mut self) -> Option<UnboundedReceiver<()>> {
        self.change_rx.take()
    }

    fn codex_running(&mut self) -> bool {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.system.processes().values().any(|process| {
            let name = process.name().to_string_lossy().to_ascii_lowercase();
            name == "codex.exe" || name == "codex" || name.starts_with("codex-")
        })
    }

    fn find_state_db(&self) -> Result<PathBuf, String> {
        let mut candidates = fs::read_dir(&self.home)
            .map_err(|error| error.to_string())?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                name.starts_with("state_") && name.ends_with(".sqlite")
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|path| {
            let name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            name.strip_prefix("state_")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0)
        });
        candidates
            .pop()
            .ok_or_else(|| "未找到兼容的 Codex 状态数据库".into())
    }

    fn read_threads(&mut self, now: i64) -> Result<Vec<ThreadState>, String> {
        let db = self.find_state_db()?;
        let conn = Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|error| error.to_string())?;
        conn.busy_timeout(Duration::from_millis(120))
            .map_err(|error| error.to_string())?;

        let columns = conn
            .prepare("PRAGMA table_info(threads)")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<HashSet<_>, _>>()
            })
            .map_err(|error| error.to_string())?;
        if !columns.contains("id") {
            return Err("threads 表结构不兼容".into());
        }

        let title = if columns.contains("title") {
            "COALESCE(title, '未命名任务')"
        } else {
            "'未命名任务'"
        };
        let rollout = if columns.contains("rollout_path") {
            "rollout_path"
        } else {
            "NULL"
        };
        let updated = if columns.contains("updated_at_ms") {
            "COALESCE(updated_at_ms, 0)"
        } else if columns.contains("updated_at") {
            "COALESCE(updated_at * 1000, 0)"
        } else {
            "0"
        };
        let archive_filter = if columns.contains("archived") {
            "WHERE archived = 0"
        } else {
            ""
        };
        let query = format!(
            "SELECT id, {title}, {rollout}, {updated} FROM threads {archive_filter} ORDER BY {updated} DESC LIMIT 16"
        );

        let raw_rows = {
            let mut statement = conn.prepare(&query).map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(|error| error.to_string())?;
            rows.flatten().collect::<Vec<_>>()
        };

        let mut threads = Vec::new();
        for (id, title, rollout, updated_at) in raw_rows {
            let reduced = rollout
                .as_deref()
                .and_then(|path| self.reduce_rollout(Path::new(path)));
            let (mut status, status_since, changed_at, confidence) = match reduced {
                Some((event, malformed)) => (
                    event.status,
                    event.at,
                    event.at,
                    if malformed {
                        ThreadConfidence::Limited
                    } else {
                        ThreadConfidence::Normal
                    },
                ),
                None => (
                    if now.saturating_sub(updated_at) < 12_000 {
                        CodexStatus::Thinking
                    } else {
                        CodexStatus::Idle
                    },
                    updated_at,
                    updated_at,
                    ThreadConfidence::Limited,
                ),
            };

            let event_age = now.saturating_sub(changed_at);
            status = match status {
                CodexStatus::Completed if event_age > COMPLETED_HOLD_MS => CodexStatus::Idle,
                CodexStatus::Failed if event_age > FAILED_HOLD_MS => CodexStatus::Idle,
                other => other,
            };

            let db_age = now.saturating_sub(updated_at);
            let should_show = match status {
                CodexStatus::Idle => db_age < 120_000,
                CodexStatus::WaitingApproval => event_age < 24 * 60 * 60_000,
                _ => event_age < 60 * 60_000,
            };
            if should_show {
                threads.push(ThreadState {
                    id,
                    title,
                    status,
                    status_since,
                    last_changed_at: changed_at,
                    confidence,
                });
            }
            if threads.len() == 8 {
                break;
            }
        }
        Ok(threads)
    }

    fn reduce_rollout(&mut self, path: &Path) -> Option<(ReducedEvent, bool)> {
        let length = fs::metadata(path).ok()?.len();
        let cache = self.rollout_cache.entry(path.to_path_buf()).or_default();
        if length < cache.offset {
            *cache = RolloutCache::default();
        }
        if length > cache.offset {
            let first_read = cache.offset == 0;
            let start = if first_read {
                length.saturating_sub(INITIAL_TAIL_BYTES)
            } else {
                cache.offset
            };
            let mut file = File::open(path).ok()?;
            file.seek(SeekFrom::Start(start)).ok()?;
            let mut text = String::new();
            file.read_to_string(&mut text).ok()?;
            cache.offset = length;

            let mut lines = text.lines();
            if first_read && start > 0 {
                lines.next();
            }
            for line in lines {
                match serde_json::from_str::<Value>(line) {
                    Ok(value) => {
                        if let Some(event) = reduce_event(&value) {
                            cache.last = Some(event);
                        }
                    }
                    Err(_) => cache.malformed_lines += 1,
                }
            }
        }
        cache
            .last
            .clone()
            .map(|event| (event, cache.malformed_lines > 0))
    }
}

impl CodexStateProvider for LocalCodexProvider {
    fn snapshot(&mut self) -> StatusSnapshot {
        let now = Utc::now().timestamp_millis();
        if !self.codex_running() {
            return StatusSnapshot {
                aggregate: CodexStatus::Offline,
                threads: vec![],
                health: ProviderHealth {
                    level: HealthLevel::Unavailable,
                    message: "Codex 未运行".into(),
                    last_successful_read: None,
                },
                observed_at: now,
            };
        }

        match self.read_threads(now) {
            Ok(threads) => {
                let limited = threads
                    .iter()
                    .any(|thread| thread.confidence == ThreadConfidence::Limited);
                let aggregate = threads
                    .iter()
                    .map(|thread| thread.status)
                    .max_by_key(|status| status.priority())
                    .unwrap_or(CodexStatus::Idle);
                StatusSnapshot {
                    aggregate,
                    threads,
                    health: ProviderHealth {
                        level: if limited {
                            HealthLevel::Degraded
                        } else {
                            HealthLevel::Healthy
                        },
                        message: if limited {
                            "部分线程仅能提供有限状态"
                        } else {
                            "状态探测正常"
                        }
                        .into(),
                        last_successful_read: Some(now),
                    },
                    observed_at: now,
                }
            }
            Err(error) => StatusSnapshot {
                aggregate: CodexStatus::Idle,
                threads: vec![],
                health: ProviderHealth {
                    level: HealthLevel::Degraded,
                    message: format!("已降级为进程探测：{error}"),
                    last_successful_read: None,
                },
                observed_at: now,
            },
        }
    }
}

fn event_timestamp(value: &Value) -> i64 {
    value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
        .map(|timestamp| timestamp.timestamp_millis())
        .unwrap_or_else(|| Utc::now().timestamp_millis())
}

fn reduce_event(value: &Value) -> Option<ReducedEvent> {
    let payload = value.get("payload")?;
    let kind = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tool = payload
        .get("name")
        .or_else(|| payload.get("tool_name"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let searchable = format!("{kind} {tool}").to_ascii_lowercase();
    let status = if searchable.contains("approval") || searchable.contains("request_user_input") {
        CodexStatus::WaitingApproval
    } else {
        match kind {
            "task_started"
            | "user_message"
            | "reasoning"
            | "function_call_output"
            | "custom_tool_call_output"
            | "patch_apply_end" => CodexStatus::Thinking,
            "function_call" | "custom_tool_call" | "patch_apply_begin" => CodexStatus::Executing,
            "task_complete" => CodexStatus::Completed,
            "turn_aborted" | "error" => CodexStatus::Failed,
            _ => return None,
        }
    };
    Some(ReducedEvent {
        status,
        at: event_timestamp(value),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: &str, name: Option<&str>) -> Value {
        serde_json::json!({
            "timestamp": "2026-07-02T00:00:00Z",
            "type": "event_msg",
            "payload": { "type": kind, "name": name }
        })
    }

    #[test]
    fn recognizes_all_product_states() {
        assert_eq!(
            reduce_event(&event("task_started", None)).unwrap().status,
            CodexStatus::Thinking
        );
        assert_eq!(
            reduce_event(&event("custom_tool_call", Some("shell_command")))
                .unwrap()
                .status,
            CodexStatus::Executing
        );
        assert_eq!(
            reduce_event(&event("custom_tool_call", Some("request_user_input")))
                .unwrap()
                .status,
            CodexStatus::WaitingApproval
        );
        assert_eq!(
            reduce_event(&event("task_complete", None)).unwrap().status,
            CodexStatus::Completed
        );
        assert_eq!(
            reduce_event(&event("error", None)).unwrap().status,
            CodexStatus::Failed
        );
    }

    #[test]
    fn malformed_lines_do_not_erase_previous_state() {
        let valid = reduce_event(&event("task_started", None));
        assert_eq!(valid.unwrap().status, CodexStatus::Thinking);
        assert!(serde_json::from_str::<Value>("not json").is_err());
    }
}
