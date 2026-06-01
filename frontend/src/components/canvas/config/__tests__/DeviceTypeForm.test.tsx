import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { DeviceTypeForm } from "@/components/canvas/config/DeviceTypeForm";
import type { ProcessorConfig } from "@/lib/canvas/types";

const DEVICE: Extract<ProcessorConfig, { type: "device_type" }> = {
  type: "device_type",
  operator: "equals",
  value: "mobile",
};

describe("DeviceTypeForm", () => {
  it("only exposes the allowed enum values", () => {
    render(<DeviceTypeForm initial={DEVICE} onChange={() => undefined} />);
    const valueSelect = screen.getByLabelText("Value") as HTMLSelectElement;
    const options = Array.from(valueSelect.options).map((o) => o.value);
    expect(options).toEqual(["mobile", "desktop", "tablet"]);
    const opSelect = screen.getByLabelText("Operator") as HTMLSelectElement;
    expect(Array.from(opSelect.options).map((o) => o.value)).toEqual([
      "equals",
      "contains",
    ]);
  });

  it("reports the updated draft as valid on change", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<DeviceTypeForm initial={DEVICE} onChange={onChange} />);
    await user.selectOptions(screen.getByLabelText("Value"), "tablet");
    const lastCall = onChange.mock.calls.at(-1);
    expect(lastCall?.[1]).toBe(true);
    expect(lastCall?.[0]).toMatchObject({ type: "device_type", value: "tablet" });
  });
});
