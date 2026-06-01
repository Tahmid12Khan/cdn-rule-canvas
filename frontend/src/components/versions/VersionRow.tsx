"use client";

// Client component: a single version table row. Owns the open state for its
// Edit-Description and Delete dialogs (triggered from RowActionsMenu). The
// version number + description link to the version detail route
// /products/features/{type}/{slug}/{vnum} (FRONTEND CONTRACT §1).
import { useState } from "react";
import Link from "next/link";

import { StatusPill } from "@/components/ui/StatusPill";
import { ConfirmDeleteDialog } from "@/components/versions/ConfirmDeleteDialog";
import { EditDescriptionDialog } from "@/components/versions/EditDescriptionDialog";
import { MakeLiveDialog } from "@/components/versions/MakeLiveDialog";
import { RowActionsMenu } from "@/components/versions/RowActionsMenu";
import type { VersionSummary } from "@/lib/api/versions";

interface VersionRowProps {
  featureId: string;
  featureType: string;
  version: VersionSummary;
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function VersionRow({
  featureId,
  featureType,
  version,
}: VersionRowProps) {
  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [makeLiveOpen, setMakeLiveOpen] = useState(false);

  const detailHref = `/products/features/${featureType}/${featureId}/${version.version_number}`;

  return (
    <tr className="border-b border-border last:border-0 hover:bg-bg-overlay">
      <td className="whitespace-nowrap px-4 py-3">
        <Link
          href={detailHref}
          className="font-mono font-semibold text-accent hover:underline"
        >
          V{version.version_number}
        </Link>
      </td>
      <td className="max-w-xs px-4 py-3">
        <Link
          href={detailHref}
          className="block truncate text-nav hover:underline"
          title={version.description ?? undefined}
        >
          {version.description ?? (
            <span className="italic text-status-prev">No description</span>
          )}
        </Link>
      </td>
      <td className="whitespace-nowrap px-4 py-3 text-status-prevFg">
        {version.last_updated_by}
      </td>
      <td
        className="whitespace-nowrap px-4 py-3 text-status-prevFg"
        suppressHydrationWarning
      >
        {formatTimestamp(version.last_updated_at)}
      </td>
      <td className="px-4 py-3">
        <StatusPill status={version.status} />
      </td>
      <td className="px-4 py-3 text-right">
        <RowActionsMenu
          featureId={featureId}
          versionNumber={version.version_number}
          status={version.status}
          onMakeLive={() => setMakeLiveOpen(true)}
          onEditDescription={() => setEditOpen(true)}
          onDelete={() => setDeleteOpen(true)}
        />
        <MakeLiveDialog
          featureId={featureId}
          versionNumber={version.version_number}
          open={makeLiveOpen}
          onOpenChange={setMakeLiveOpen}
        />
        <EditDescriptionDialog
          featureId={featureId}
          versionNumber={version.version_number}
          initialDescription={version.description}
          open={editOpen}
          onOpenChange={setEditOpen}
        />
        <ConfirmDeleteDialog
          featureId={featureId}
          versionNumber={version.version_number}
          open={deleteOpen}
          onOpenChange={setDeleteOpen}
        />
      </td>
    </tr>
  );
}
