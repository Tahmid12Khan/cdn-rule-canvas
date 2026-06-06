import type { TestPresetRead } from "@/lib/api/test-presets";

// Test-preset card. Presentational: shows the preset name, a slug badge, a kind
// badge (rule | url), and a short payload summary. Actions are wired by the list
// client. Mirrors SiteCard.

// Build a compact one-line summary of the payload's salient fields so the user
// can tell presets apart at a glance.
function payloadSummary(
  kind: TestPresetRead["kind"],
  payload: Record<string, unknown>,
): string {
  const parts: string[] = [];
  if (kind === "url") {
    if (typeof payload.url === "string" && payload.url) parts.push(payload.url);
  } else {
    if (typeof payload.device_type === "string")
      parts.push(payload.device_type);
    if (typeof payload.path === "string" && payload.path)
      parts.push(payload.path);
    if (typeof payload.site === "string" && payload.site)
      parts.push(`site:${payload.site}`);
  }
  if (payload.headers && typeof payload.headers === "object") {
    const n = Object.keys(payload.headers as object).length;
    if (n > 0) parts.push(`${n} ${n === 1 ? "header" : "headers"}`);
  }
  return parts.length > 0 ? parts.join(" · ") : "No inputs set";
}

interface TestPresetCardProps {
  preset: TestPresetRead;
  onEdit: (preset: TestPresetRead) => void;
  onDelete: (preset: TestPresetRead) => void;
}

export function TestPresetCard({
  preset,
  onEdit,
  onDelete,
}: TestPresetCardProps) {
  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border bg-bg-elevated p-5">
      <div className="flex items-start justify-between gap-3">
        <h3 className="text-base font-semibold text-nav">{preset.name}</h3>
        <code className="shrink-0 rounded-full bg-brand-50 px-2 py-0.5 font-mono text-xs text-accent-onMuted">
          {preset.slug}
        </code>
      </div>

      <div className="flex items-center gap-2">
        <span className="self-start rounded-full bg-brand-50 px-2 py-0.5 text-xs font-medium text-accent-onMuted">
          {preset.kind}
        </span>
        <span className="truncate text-xs text-fg-muted">
          {payloadSummary(preset.kind, preset.payload)}
        </span>
      </div>

      <div className="mt-1 flex items-center gap-2">
        <button
          type="button"
          onClick={() => onEdit(preset)}
          className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-nav transition hover:bg-status-prevBg/30"
        >
          Edit
        </button>
        <button
          type="button"
          onClick={() => onDelete(preset)}
          className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-danger transition hover:bg-danger/10"
        >
          Delete
        </button>
      </div>
    </div>
  );
}
