// Plain-English "what was done" sentence for one Transformation Journey step
// (features-matched-spec §8). Data-driven from the live canvas node's config
// (the `{ type, …fields }` ProcessorConfig) plus the journey step's
// kind/label/branch. Pure; no React.
import type { ProcessorConfig } from "@/lib/canvas/types";

function str(value: unknown): string {
  if (value === undefined || value === null) return "";
  return String(value);
}

// `kind` is the journey step kind (start/decision/expression/end); `config` is
// the live canvas node's processor (decision) or action (expression) config, or
// undefined when the node isn't in the live canvas. `branch` is the decision's
// taken branch (true/false) or null. `label` is the display name used for the
// outcome banner phrasing (apply_outcome) and the component name phrasing
// (apply_component / apply_component_json) — both resolved on deserialize.
export function describeStep(
  kind: string,
  config: ProcessorConfig | undefined,
  branch: boolean | null,
  label: string,
): string {
  if (kind === "start") return "Start of the flow.";
  if (kind === "end")
    return "End of the flow — this is the final output.";

  const actionType = config?.type;

  if (kind === "expression") {
    if (actionType === "trim_json") {
      const path = str(config?.json_path) || "$";
      const length = str(config?.length) || "0";
      return `Trimmed the array at \`${path}\` to at most ${length} items.`;
    }
    if (actionType === "add_attribute") {
      const path = str(config?.json_path) || "$";
      const value = str(config?.value);
      return `Set \`${path}\` to \`${value}\`.`;
    }
    if (actionType === "apply_outcome") {
      return `Applied the outcome ‘${label}’.`;
    }
    if (actionType === "apply_component") {
      // `label` carries the resolved component name (falls back to the manifest
      // label when not yet resolved). `target_selector` + placement_mode come
      // from the action config.
      const selector = str(config?.target_selector) || "the page";
      const placement = str(config?.placement_mode) || "append";
      return `Rendered the component ‘${label}’ and applied it (${placement}) at \`${selector}\`.`;
    }
    if (actionType === "apply_component_json") {
      const path = str(config?.target_path) || "$";
      return `Rendered the component ‘${label}’ and set it at \`${path}\`.`;
    }
    // Fallback for an action kind without bespoke phrasing.
    return `Ran ${label}.`;
  }

  if (kind === "decision") {
    // The condition fields vary by processor (json_expression uses json_path;
    // device_type/meta_tags/article_url use value, etc.). Pick the most
    // descriptive subject field available.
    const subject =
      str(config?.json_path) ||
      str(config?.tag_name) ||
      "the input";
    const operator = str(config?.operator) || "matches";
    const value = str(config?.value);
    const result = branch === null ? "" : branch ? " → matched (yes)" : " → did not match (no)";
    const valuePart = value ? ` \`${value}\`` : "";
    return `Checked \`${subject}\` ${operator}${valuePart}${result}.`;
  }

  return label;
}
