import { describe, expect, it } from "vitest";
import { aggregateStatus, formatDuration } from "./status";
import type { ThreadState } from "./types";

const thread = (status: ThreadState["status"]): ThreadState => ({ id: status, title: status, status, since: 0, lastChangedAt: 0 });

describe("aggregateStatus", () => {
  it("returns offline when Codex is not running", () => expect(aggregateStatus([thread("failed")], false)).toBe("offline"));
  it("returns idle for no active threads", () => expect(aggregateStatus([], true)).toBe("idle"));
  it("uses attention priority", () => expect(aggregateStatus([thread("executing"), thread("waitingApproval"), thread("completed")], true)).toBe("waitingApproval"));
  it("makes failures highest priority", () => expect(aggregateStatus([thread("failed"), thread("waitingApproval")], true)).toBe("failed"));
});

describe("formatDuration", () => {
  it("formats seconds and minutes", () => {
    expect(formatDuration(9_000, 10_000)).toBe("1 秒");
    expect(formatDuration(0, 120_000)).toBe("2 分钟");
  });
});
