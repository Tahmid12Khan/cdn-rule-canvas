// Palette chip definitions (Task 12). The palette renders every §4.5 category,
// but only a small subset is creatable for MVP (Content → Meta Tags, Session →
// Device Type, and one chip per Outcome of the current version). Everything
// else renders as a disabled "Coming soon" chip.
import type { ProcessorConfig } from "@/lib/canvas/types";

// What a chip drops onto the canvas. Decision chips carry a default processor
// config (intentionally requiring user edit before save); outcome chips carry
// the outcome id + cached title.
export type ChipPayload =
  | { kind: "decision"; processor: ProcessorConfig }
  | { kind: "outcome"; outcomeId: string; title: string };

export interface NodeChipDef {
  id: string; // stable chip id (used as DnD payload key + React key)
  label: string;
  enabled: boolean;
  payload?: ChipPayload; // present only when enabled
}

export interface PaletteCategory {
  id: string;
  label: string;
  chips: NodeChipDef[];
}

// Default processor configs for the two MVP decision processors. These are
// intentionally "incomplete" (empty value / blank tag) so the SaveBar/backend
// validation nudges the user to open the config drawer first.
export const DEFAULT_META_TAGS: Extract<ProcessorConfig, { type: "meta_tags" }> =
  {
    type: "meta_tags",
    tag_name: "",
    operator: "contains",
    value: "",
  };

export const DEFAULT_DEVICE_TYPE: Extract<
  ProcessorConfig,
  { type: "device_type" }
> = {
  type: "device_type",
  operator: "equals",
  value: "mobile",
};

export const DEFAULT_ARTICLE_URL: Extract<
  ProcessorConfig,
  { type: "article_url" }
> = {
  type: "article_url",
  operator: "contains",
  value: "",
};

// Category order matches FRONTEND CONTRACT / Task 12 §4.5 (search tab leads,
// rendered separately by the palette). "Outcomes" is populated dynamically.
const STATIC_CATEGORIES: PaletteCategory[] = [
  {
    id: "session",
    label: "Session",
    chips: [
      {
        id: "session:device-type",
        label: "Device Type",
        enabled: true,
        payload: { kind: "decision", processor: DEFAULT_DEVICE_TYPE },
      },
    ],
  },
  {
    id: "user",
    label: "User",
    chips: [{ id: "user:logged-in", label: "Logged In", enabled: false }],
  },
  {
    id: "content",
    label: "Content",
    chips: [
      {
        id: "content:meta-tags",
        label: "Meta Tags",
        enabled: true,
        payload: { kind: "decision", processor: DEFAULT_META_TAGS },
      },
      {
        id: "content:article-url",
        label: "Article URL",
        enabled: true,
        payload: { kind: "decision", processor: DEFAULT_ARTICLE_URL },
      },
    ],
  },
  {
    id: "decision-data",
    label: "Decision Data",
    chips: [{ id: "decision-data:custom", label: "Custom Field", enabled: false }],
  },
  {
    id: "access",
    label: "Access",
    chips: [{ id: "access:entitlement", label: "Entitlement", enabled: false }],
  },
  {
    id: "sub-rules",
    label: "Sub Rules",
    chips: [{ id: "sub-rules:reusable", label: "Reusable Rule", enabled: false }],
  },
  {
    id: "split-tests",
    label: "Split Tests",
    chips: [{ id: "split-tests:ab", label: "A/B Test", enabled: false }],
  },
  {
    id: "integrations",
    label: "Integrations",
    chips: [{ id: "integrations:webhook", label: "Webhook", enabled: false }],
  },
  // "outcomes" injected dynamically by buildPalette()
  {
    id: "advanced",
    label: "Advanced",
    chips: [{ id: "advanced:script", label: "Script", enabled: false }],
  },
  {
    id: "bypass",
    label: "Bypass",
    chips: [{ id: "bypass:rule", label: "Bypass Rule", enabled: false }],
  },
  {
    id: "gift-tokens",
    label: "Gift Tokens",
    chips: [{ id: "gift-tokens:grant", label: "Grant Token", enabled: false }],
  },
  {
    id: "campaign-tokens",
    label: "Campaign Tokens",
    chips: [
      { id: "campaign-tokens:apply", label: "Apply Campaign", enabled: false },
    ],
  },
  {
    id: "custom-segments",
    label: "Custom Segments",
    chips: [{ id: "custom-segments:segment", label: "Segment", enabled: false }],
  },
];

export interface OutcomeOption {
  id: string;
  title: string;
}

// Build the full palette for a version, injecting one draggable chip per
// outcome into the "Outcomes" category (placed in §4.5 order between
// Integrations and Advanced).
export function buildPalette(outcomes: OutcomeOption[]): PaletteCategory[] {
  const outcomesCategory: PaletteCategory = {
    id: "outcomes",
    label: "Outcomes",
    chips:
      outcomes.length === 0
        ? [{ id: "outcomes:none", label: "No outcomes yet", enabled: false }]
        : outcomes.map((o) => ({
            id: `outcomes:${o.id}`,
            label: o.title,
            enabled: true,
            payload: { kind: "outcome", outcomeId: o.id, title: o.title },
          })),
  };

  const result: PaletteCategory[] = [];
  for (const cat of STATIC_CATEGORIES) {
    if (cat.id === "advanced") result.push(outcomesCategory);
    result.push(cat);
  }
  return result;
}

// DataTransfer MIME type for palette drag payloads.
export const CHIP_MIME = "application/rre-chip";
