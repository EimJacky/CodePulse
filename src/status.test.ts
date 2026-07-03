import { describe, expect, it } from "vitest";
import { aggregateStatus, formatDuration, formatRelative, statusPriority } from "./status";
import type { ThreadState } from "./types";

const thread = (status: ThreadState["status"]): ThreadState => ({
  id: status,
  title: status,
  status,
  statusSince: 0,
  lastChangedAt: 0,
  confidence: "normal"
});

describe("aggregateStatus", () => {
  it("returns offline when Codex is not running", () => expect(aggregateStatus([thread("failed")], false)).toBe("offline"));
  it("returns idle for no active threads", () => expect(aggregateStatus([], true)).toBe("idle"));
  it("uses the product attention priority", () => {
    expect(aggregateStatus([thread("thinking"), thread("executing"), thread("waitingApproval")], true)).toBe("waitingApproval");
    expect(statusPriority.failed).toBeGreaterThan(statusPriority.waitingApproval);
  });
  it("makes failures highest priority", () => expect(aggregateStatus([thread("failed"), thread("waitingApproval")], true)).toBe("failed"));
});

describe("time labels", () => {
  it("formats status duration", () => {
    expect(formatDuration(9_000, 10_000)).toBe("01秒");
    expect(formatDuration(0, 120_000)).toBe("02:00");
  });
  it("formats recent changes", () => {
    expect(formatRelative(8_000, 10_000)).toBe("刚刚");
    expect(formatRelative(0, 75_000)).toBe("1分钟前");
  });
});