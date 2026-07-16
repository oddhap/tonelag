import { describe, expect, it } from "vitest";
import { formatTime } from "../src/lib/format";

describe("formatTime", () => {
  it("formats whole minutes and seconds", () => expect(formatTime(125_900)).toBe("02:05"));
  it("shows unknown durations", () => expect(formatTime(null)).toBe("--:--"));
});
