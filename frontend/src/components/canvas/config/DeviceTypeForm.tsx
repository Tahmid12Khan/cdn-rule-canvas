"use client";

// Device Type processor config form (Task 13). Operator + value are both
// constrained selects (only allowed enum values). Always valid once both are
// chosen (defaults are valid), so it primarily reports the draft upward.
import { useEffect, useState } from "react";

import {
  deviceTypeSchema,
  type DeviceTypeFormValues,
} from "@/lib/canvas/processorSchemas";
import type {
  DeviceOperator,
  DeviceValue,
  ProcessorConfig,
} from "@/lib/canvas/types";

interface DeviceTypeFormProps {
  initial: Extract<ProcessorConfig, { type: "device_type" }>;
  onChange: (draft: ProcessorConfig, valid: boolean) => void;
  // View-only mode (Task D): selects render disabled so the node's contents are
  // inspectable without being editable.
  disabled?: boolean;
}

const OPERATORS: DeviceOperator[] = ["equals", "contains"];
const VALUES: DeviceValue[] = ["mobile", "desktop", "tablet"];

export function DeviceTypeForm({
  initial,
  onChange,
  disabled = false,
}: DeviceTypeFormProps) {
  const [operator, setOperator] = useState<DeviceOperator>(initial.operator);
  const [value, setValue] = useState<DeviceValue>(initial.value);

  const draft: DeviceTypeFormValues = {
    type: "device_type",
    operator,
    value,
  };
  const valid = deviceTypeSchema.safeParse(draft).success;

  useEffect(() => {
    onChange(draft, valid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [operator, value]);

  return (
    <div className="space-y-4">
      <div>
        <label
          htmlFor="dt-operator"
          className="mb-1 block text-sm font-medium text-nav"
        >
          Operator
        </label>
        <select
          id="dt-operator"
          value={operator}
          onChange={(e) => setOperator(e.target.value as DeviceOperator)}
          disabled={disabled}
          className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {OPERATORS.map((op) => (
            <option key={op} value={op}>
              {op}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label
          htmlFor="dt-value"
          className="mb-1 block text-sm font-medium text-nav"
        >
          Value
        </label>
        <select
          id="dt-value"
          value={value}
          onChange={(e) => setValue(e.target.value as DeviceValue)}
          disabled={disabled}
          className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {VALUES.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
