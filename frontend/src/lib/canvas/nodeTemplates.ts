// Palette chip definitions (spec Part E). The palette is fully data-driven from
// the node-type manifest (categories + node_types). Each enabled decision chip
// carries a default processor config built from the spec's field defaults;
// disabled chips come from categories flagged `coming_soon` (and from categories
// with no node types yet). Outcome chips remain dynamic (one per version
// outcome). Adding a node type needs ZERO frontend change.
import type { NodeCategory, NodeManifest, NodeTypeSpec } from "@/lib/api/nodeTypes";
import { defaultProcessor } from "@/lib/canvas/manifest";
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

export interface OutcomeOption {
  id: string;
  title: string;
}

// Build one chip per node-type spec in a category. A `coming_soon` category
// renders all its chips disabled (even though they map to real specs); the
// dropped-config default is built from the spec's field defaults.
function chipForSpec(spec: NodeTypeSpec, comingSoon: boolean): NodeChipDef {
  if (comingSoon) {
    return { id: `node:${spec.kind}`, label: spec.label, enabled: false };
  }
  return {
    id: `node:${spec.kind}`,
    label: spec.label,
    enabled: true,
    payload: { kind: "decision", processor: defaultProcessor(spec) },
  };
}

// A category with no node types yet still renders (as a disabled placeholder)
// so the palette mirrors the manifest's full category list.
function placeholderChip(category: NodeCategory): NodeChipDef {
  return {
    id: `${category.id}:coming-soon`,
    label: "Coming soon",
    enabled: false,
  };
}

// Build the full palette from the manifest + the version's outcomes. Categories
// render in manifest order; the dynamic "Outcomes" category is injected before
// the first category that has no node types after Integrations (mirroring the
// prior §4.5 ordering: between Integrations and Advanced).
export function buildPalette(
  manifest: NodeManifest | undefined,
  outcomes: OutcomeOption[],
): PaletteCategory[] {
  if (!manifest) return [];

  const specsByCategory = new Map<string, NodeTypeSpec[]>();
  for (const spec of manifest.node_types) {
    const list = specsByCategory.get(spec.category) ?? [];
    list.push(spec);
    specsByCategory.set(spec.category, list);
  }

  const categories: PaletteCategory[] = manifest.categories.map((cat) => {
    const comingSoon = cat.coming_soon === true;
    const specs = specsByCategory.get(cat.id) ?? [];
    const chips =
      specs.length > 0
        ? specs.map((s) => chipForSpec(s, comingSoon))
        : [placeholderChip(cat)];
    return { id: cat.id, label: cat.label, chips };
  });

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

  // Inject "Outcomes" before the "advanced" category (matching prior ordering);
  // fall back to appending if that category isn't present.
  const advancedIdx = categories.findIndex((c) => c.id === "advanced");
  if (advancedIdx === -1) {
    return [...categories, outcomesCategory];
  }
  return [
    ...categories.slice(0, advancedIdx),
    outcomesCategory,
    ...categories.slice(advancedIdx),
  ];
}

// DataTransfer MIME type for palette drag payloads.
export const CHIP_MIME = "application/rre-chip";
