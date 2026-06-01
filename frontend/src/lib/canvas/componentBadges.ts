import { ComponentConfig } from "@/lib/schemas/components";

// componentBadges (FRONTEND CONTRACT §2.5). Derives a small set of display
// badges from a component's config so the <ComponentRow/> and the config modal
// header can summarize a component at a glance.
//
// `config` arrives from the API as an unknown JSON value (BACKEND serializes it
// as serde_json::Value); we re-parse it with the discriminated ComponentConfig
// schema and fall back gracefully when it does not match.

export interface ComponentBadge {
  label: string;
  tone: "type" | "placement" | "info";
}

const TYPE_LABELS: Record<ComponentConfig["type"], string> = {
  html_injection: "HTML Injection",
  content_truncation: "Content Truncation",
};

const PLACEMENT_MODE_LABELS: Record<string, string> = {
  replace: "Replace",
  append: "Append",
  prepend: "Prepend",
  before: "Before",
  after: "After",
};

export function componentBadges(config: unknown): ComponentBadge[] {
  const parsed = ComponentConfig.safeParse(config);
  if (!parsed.success) {
    return [{ label: "Unknown", tone: "info" }];
  }

  const c = parsed.data;
  const badges: ComponentBadge[] = [{ label: TYPE_LABELS[c.type], tone: "type" }];

  if (c.type === "html_injection") {
    const mode = PLACEMENT_MODE_LABELS[c.placement_mode] ?? c.placement_mode;
    badges.push({ label: mode, tone: "placement" });
    if (c.target_selector) {
      badges.push({ label: c.target_selector, tone: "info" });
    }
  } else {
    badges.push({ label: `${c.word_count} words`, tone: "info" });
    if (c.fade_out) {
      badges.push({ label: "Fade out", tone: "placement" });
    }
  }

  return badges;
}
