import type { SiteRead } from "@/lib/api/sites";

// Site card (spec §6). Presentational: shows the source → destination routing
// for one Site plus Edit / Delete actions. The actions are wired by the list
// client; the card itself stays a plain client-safe leaf (rendered inside the
// "use client" SitesListClient).

function authority(protocol: string, host: string, port: number): string {
  return `${protocol}://${host}:${port}`;
}

interface SiteCardProps {
  site: SiteRead;
  onEdit: (site: SiteRead) => void;
  onDelete: (site: SiteRead) => void;
}

export function SiteCard({ site, onEdit, onDelete }: SiteCardProps) {
  const headerCount = Object.keys(site.headers).length;
  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border bg-bg-elevated p-5">
      <div className="flex items-start justify-between gap-3">
        <h3 className="text-base font-semibold text-nav">{site.name}</h3>
        <code className="shrink-0 rounded-full bg-brand-50 px-2 py-0.5 font-mono text-xs text-accent-onMuted">
          {site.slug}
        </code>
      </div>

      <div className="flex flex-col gap-1 text-xs">
        <code className="truncate font-mono text-fg-muted">
          {authority(site.source_protocol, site.source_host, site.source_port)}
        </code>
        <span className="text-status-prevFg" aria-hidden>
          ↓
        </span>
        <code className="truncate font-mono text-fg-muted">
          {authority(site.dest_protocol, site.dest_host, site.dest_port)}
        </code>
      </div>

      {headerCount > 0 && (
        <span className="self-start rounded-full bg-brand-50 px-2 py-0.5 text-xs font-medium text-accent-onMuted">
          {headerCount} {headerCount === 1 ? "header" : "headers"}
        </span>
      )}

      <div className="mt-1 flex items-center gap-2">
        <button
          type="button"
          onClick={() => onEdit(site)}
          className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-nav transition hover:bg-status-prevBg/30"
        >
          Edit
        </button>
        <button
          type="button"
          onClick={() => onDelete(site)}
          className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-danger transition hover:bg-danger/10"
        >
          Delete
        </button>
      </div>
    </div>
  );
}
