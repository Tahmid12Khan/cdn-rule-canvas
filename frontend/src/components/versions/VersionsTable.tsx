// Versions table (FRONTEND CONTRACT §2.3). Columns: Version, Description,
// Created By, Last Updated, Status, Actions. Pure presentational table — the
// interactive bits live in VersionRow (client). Renderable on the server.
import { VersionRow } from "@/components/versions/VersionRow";
import type { VersionSummary } from "@/lib/api/versions";

interface VersionsTableProps {
  featureId: string;
  featureType: string;
  versions: VersionSummary[];
}

const HEADERS = [
  "Version",
  "Description",
  "Created By",
  "Last Updated",
  "Status",
] as const;

export function VersionsTable({
  featureId,
  featureType,
  versions,
}: VersionsTableProps) {
  if (versions.length === 0) {
    return (
      <div className="rounded-xl border border-dashed border-status-prevBg bg-bg-elevated px-6 py-12 text-center text-sm text-status-prevFg">
        No versions match your search.
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-bg-elevated">
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="border-b border-border bg-bg-overlay text-xs font-semibold uppercase tracking-wide text-fg-muted">
            {HEADERS.map((h) => (
              <th key={h} scope="col" className="px-4 py-3">
                {h}
              </th>
            ))}
            <th scope="col" className="px-4 py-3 text-right">
              <span className="sr-only">Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {versions.map((version) => (
            <VersionRow
              key={version.id}
              featureId={featureId}
              featureType={featureType}
              version={version}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}
