import { describe, expect, it } from "vitest";
import { formatHours, totalHours } from "./harvest";
import type { HarvestTimeEntry } from "../bindings";

function entry(
  hours: number,
  partial: Partial<HarvestTimeEntry> = {},
): HarvestTimeEntry {
  return {
    id: partial.id ?? Math.round(hours * 1000),
    hours,
    notes: partial.notes ?? null,
    project: partial.project ?? "Project",
    client: partial.client ?? "Client",
    task: partial.task ?? "Task",
    is_running: partial.is_running ?? false,
    url: partial.url ?? null,
  };
}

describe("formatHours", () => {
  it("renders whole hours with zero minutes", () => {
    expect(formatHours(7)).toBe("7:00");
    expect(formatHours(0)).toBe("0:00");
  });

  it("converts decimal fractions to minutes", () => {
    expect(formatHours(0.5)).toBe("0:30");
    expect(formatHours(2.25)).toBe("2:15");
    expect(formatHours(2.11)).toBe("2:07");
  });

  it("rounds to the nearest minute and carries into hours", () => {
    expect(formatHours(1.9999)).toBe("2:00");
    expect(formatHours(0.99)).toBe("0:59");
  });

  it("clamps negative and non-finite input", () => {
    expect(formatHours(-1)).toBe("0:00");
    expect(formatHours(Number.NaN)).toBe("0:00");
  });
});

describe("totalHours", () => {
  it("is zero for no entries", () => {
    expect(totalHours([])).toBe(0);
  });

  it("sums hours across entries including running timers", () => {
    const total = totalHours([
      entry(2.5),
      entry(1.25),
      entry(0.25, { is_running: true }),
    ]);
    expect(total).toBeCloseTo(4);
  });
});
