import type { CodexStatus, ThreadState } from "./types";

export const statusMeta: Record<CodexStatus, { label: string; detail: string }> = {
  offline: { label: "离线", detail: "未检测到 Codex" },
  idle: { label: "空闲", detail: "等待新的任务" },
  thinking: { label: "思考中", detail: "正在分析与规划" },
  executing: { label: "执行中", detail: "正在执行任务" },
  waitingApproval: { label: "等待确认", detail: "需要你的确认" },
  completed: { label: "已完成", detail: "任务处理完成" },
  failed: { label: "执行失败", detail: "任务需要处理" }
};

export const statusPriority: Record<CodexStatus, number> = {
  offline: 0,
  idle: 1,
  completed: 2,
  thinking: 3,
  executing: 4,
  waitingApproval: 5,
  failed: 6
};

export function aggregateStatus(threads: ThreadState[], running: boolean): CodexStatus {
  if (!running) return "offline";
  if (!threads.length) return "idle";
  return threads.reduce<CodexStatus>(
    (highest, thread) => statusPriority[thread.status] > statusPriority[highest] ? thread.status : highest,
    "idle"
  );
}

export function formatDuration(since: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor((now - since) / 1000));
  if (seconds < 60) return `${String(seconds).padStart(2, "0")}秒`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
  return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

export function formatRelative(timestamp: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor((now - timestamp) / 1000));
  if (seconds < 5) return "刚刚";
  if (seconds < 60) return `${seconds}秒前`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}分钟前`;
  return `${Math.floor(minutes / 60)}小时前`;
}