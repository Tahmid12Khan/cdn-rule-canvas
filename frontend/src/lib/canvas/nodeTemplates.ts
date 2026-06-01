// Palette chip definitions (spec Part E). The palette is fully data-driven from
// the node-type manifest (categories + node_types). Each enabled decision chip
// carries a default processor config built from the spec's field defaults;
// disabled chips come from categories flagged `coming_soon` (and from categories
// with no node types yet). Outcome chips remain dynamic (one per version
// outcome). Adding a node type needs ZERO frontend change.
import type {
  NodeAppliesTo,
  NodeCategory,
  NodeManifest,
  NodeTypeSpec,
} from "@/lib/api/nodeTypes";
import { defaultProcessor, rfNodeTypeForSpec } from "@/lib/canvas/manifest";
import type { ProcessorConfig } from "@/lib/canvas/types";

// The feature's content kind. A node spec's `applies_to` gates palette
// availability: "all"/undefined shows everywhere; "html"/"json" only for that
// feature type. (spec §1.2 / req 7)
export type FeatureType = "html" | "json";

// True when a node spec should appear in the palette for the given feature type.
function specAppliesTo(
  appliesTo: NodeAppliesTo | undefined,
  featureType: FeatureType,
): boolean {
  return appliesTo === undefined || appliesTo === "all" || appliesTo === featureType;
}

// What a chip drops onto the canvas. Driven by the spec's manifest `node_kind`
// (expression-nodes-spec §2): a decision chip carries a default processor
// config; an expression chip (Trim JSON / Add Attribute / Apply Outcome)
// carries a default action config. Both are intentionally incomplete (the user
// must edit before the graph validates).
export type ChipPayload =
  | { kind: "decision"; processor: ProcessorConfig }
  | { kind: "expression"; action: ProcessorConfig };

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
// dropped-config default is built from the spec's field defaults. The chip
// payload kind follows the spec's manifest `node_kind` (decision vs expression).
function chipForSpec(spec: NodeTypeSpec, comingSoon: boolean): NodeChipDef {
  if (comingSoon) {
    return { id: `node:${spec.kind}`, label: spec.label, enabled: false };
  }
  const config = defaultProcessor(spec);
  const payload: ChipPayload =
    rfNodeTypeForSpec(spec) === "expressionNode"
      ? { kind: "expression", action: config }
      : { kind: "decision", processor: config };
  return {
    id: `node:${spec.kind}`,
    label: spec.label,
    enabled: true,
    payload,
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

// Build the full palette from the manifest. Categories render in manifest
// order. Saved outcomes are no longer their own palette category — "Apply
// Outcome" is a manifest expression node (content category) and the specific
// outcome is chosen via the outcome_select dropdown in its config drawer
// (expression-nodes-spec §2). The `outcomes` arg is retained for API parity
// (the caller still threads it) but no longer affects the palette.
export function buildPalette(
  manifest: NodeManifest | undefined,
  _outcomes: OutcomeOption[],
  featureType: FeatureType = "html",
): PaletteCategory[] {
  if (!manifest) return [];

  const specsByCategory = new Map<string, NodeTypeSpec[]>();
  for (const spec of manifest.node_types) {
    // Filter by feature type: a json_expression node only shows for JSON
    // features, meta_tags only for HTML, request-based nodes ("all") always.
    if (!specAppliesTo(spec.applies_to, featureType)) continue;
    const list = specsByCategory.get(spec.category) ?? [];
    list.push(spec);
    specsByCategory.set(spec.category, list);
  }

  // A category that ends up empty after filtering (all its specs were gated out)
  // is dropped entirely so we don't render a misleading "Coming soon"
  // placeholder for it. A category that NEVER had specs still renders its
  // placeholder (mirrors the manifest's full category list).
  const hadSpecs = new Set(manifest.node_types.map((s) => s.category));

  const categories: PaletteCategory[] = manifest.categories
    .filter(
      (cat) => !hadSpecs.has(cat.id) || specsByCategory.has(cat.id),
    )
    .map((cat) => {
      const comingSoon = cat.coming_soon === true;
      const specs = specsByCategory.get(cat.id) ?? [];
      const chips =
        specs.length > 0
          ? specs.map((s) => chipForSpec(s, comingSoon))
          : [placeholderChip(cat)];
      return { id: cat.id, label: cat.label, chips };
    });

  return categories;
}

// DataTransfer MIME type for palette drag payloads.
export const CHIP_MIME = "application/rre-chip";
