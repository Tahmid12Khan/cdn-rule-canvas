import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import {
  JsonMutationForm,
  type JsonMutationConfig,
} from "@/components/component-config/JsonMutationForm";
import { defaultConfigFor } from "@/lib/schemas/components";

const FORM_ID = "jm-test-form";

// defaultConfigFor has per-literal overloads; the union arg would widen to
// ComponentConfig, so narrow explicitly to keep the JsonMutationConfig type.
function jsonDefault(
  type: "json_remove" | "json_set" | "json_replace",
): JsonMutationConfig {
  if (type === "json_remove") return defaultConfigFor("json_remove");
  if (type === "json_set") return defaultConfigFor("json_set");
  return defaultConfigFor("json_replace");
}

function renderForm(
  type: "json_remove" | "json_set" | "json_replace",
  onValidSubmit: (c: JsonMutationConfig) => void,
) {
  return render(
    <>
      <JsonMutationForm
        formId={FORM_ID}
        type={type}
        initial={jsonDefault(type)}
        onValidSubmit={onValidSubmit}
      />
      <button type="submit" form={FORM_ID}>
        Save
      </button>
    </>,
  );
}

describe("JsonMutationForm", () => {
  it("json_remove submits with just a target_path", async () => {
    const user = userEvent.setup();
    const onValid = vi.fn();
    renderForm("json_remove", onValid);

    // No value editor for remove.
    expect(screen.queryByLabelText(/value \(json\)/i)).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Target path"), "$.user.premium");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onValid).toHaveBeenCalledWith({
      type: "json_remove",
      target_path: "$.user.premium",
    });
  });

  it("blocks submit and shows a path error when target_path is empty", async () => {
    const user = userEvent.setup();
    const onValid = vi.fn();
    renderForm("json_set", onValid);

    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onValid).not.toHaveBeenCalled();
    expect(screen.getByText(/enter a json path/i)).toBeInTheDocument();
  });

  it("json_set parses the JSON value editor and submits the parsed value", async () => {
    const user = userEvent.setup();
    const onValid = vi.fn();
    renderForm("json_set", onValid);

    await user.type(screen.getByLabelText("Target path"), "$.tier");
    await user.type(screen.getByLabelText(/value \(json\)/i), '"gold"');
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onValid).toHaveBeenCalledWith({
      type: "json_set",
      target_path: "$.tier",
      value: "gold",
    });
  });

  it("shows an inline parse error for an invalid JSON value", async () => {
    const user = userEvent.setup();
    const onValid = vi.fn();
    renderForm("json_replace", onValid);

    await user.type(screen.getByLabelText("Target path"), "$.tier");
    // `{` and `[` are special in userEvent keyboard syntax — escape them.
    await user.type(screen.getByLabelText(/value \(json\)/i), "{{not json");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onValid).not.toHaveBeenCalled();
    expect(screen.getByText(/isn't valid json/i)).toBeInTheDocument();
  });
});
