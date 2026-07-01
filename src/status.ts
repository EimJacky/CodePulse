import type { CodexStatus, ThreadState } from "./types";

export const statusMeta: Record<CodexStatus, { label: string; detail: string }> = {
  offline: { label: "离线", detail: "Codex 未运行" },
  idle: { label: "已就绪", detail: "等待新任务" },
  thinking: { label: "思考中", detail: "正在理解任务" },
  executing: { label: "工作中", detail: "正在执行操作" },
  waitingApproval: { label: "等待确认", detail: "需要你的操作" },
  completed: { label: "已完成", detail: "任务处理完成" },
  failed: { label: "出现问题", detail: "任务执行失败" }
};

const priority: Record<CodexStatus, number> = {
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
    (highest, thread) => priority[thread.status] > priority[highest] ? thread.status : highest,
    "idle"
  );
}

export function formatDuration(since: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor((now - since) / 1000));
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟`;
  return `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分钟`;
}
