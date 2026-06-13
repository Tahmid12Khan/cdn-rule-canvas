import Link from "next/link";

import type { ComponentTemplateSummary } from "@/lib/schemas/componentTemplates";

// Component library card. Presentational: name, slug badge, optional
// description, and a default/latest version summary. The whole card links to
// the editor; the Delete action is wired by the list client. Mirrors
// TestPresetCard / SiteCard.
interface ComponentCardProps {
  component: ComponentTemplateSummary;
  onDelete: (component: ComponentTemplateSummary) => void;
}

export function ComponentCard({ component, onDelete }: ComponentCardProps) {
  const defaultLabel =
    component.default_version_number != null
      ? `default v${component.default_version_number}`
      : "no default";

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border bg-bg-elevated p-5">
      <div className="flex items-start justify-between gap-3">
        <Link
          href={`/products/components/${component.slug}`}
          className="text-base font-semibold text-fg hover:text-accent-onMuted"
        >
          {component.name}
        </Link>
        <code className="shrink-0 rounded-full bg-bg-overlay px-2 py-0.5 font-mono text-xs text-accent-onMuted">
          {component.slug}
        </code>
      </div>

      {component.description && (
        <p className="line-clamp-2 text-xs text-fg-muted">
          {component.description}
        </p>
      )}

      <div className="flex items-center gap-2 text-xs text-fg-muted">
        <span className="rounded-full bg-bg-overlay px-2 py-0.5 font-medium">
          {defaultLabel}
        </span>
        <span>latest v{component.latest_version_number}</span>
      </div>

      <div className="mt-1 flex items-center gap-2">
        <Link
          href={`/products/components/${component.slug}`}
          className="rounded-md border border-border px-3 py-1 text-xs font-medium text-fg transition hover:bg-bg-overlay"
        >
          Edit
        </Link>
        <button
          type="button"
          onClick={() => onDelete(component)}
          className="rounded-md border border-border px-3 py-1 text-xs font-medium text-danger transition hover:bg-danger/10"
        >
          Delete
        </button>
      </div>
    </div>
  );
}
