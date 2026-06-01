// Deployment status row (FRONTEND CONTRACT §2.3). Shows the current STAGING and
// LIVE deployment pills, resolving the feature's staging/live version UUIDs to
// their version numbers via the loaded version summaries. Server-renderable —
// no client-only APIs.
import clsx from "clsx";

interface DeploymentStatusRowProps {
  stagingVersionId: string | null;
  liveVersionId: string | null;
  // id -> version_number, resolved from the versions list query.
  versionNumberById: (id: string) => number | undefined;
}

function DeploymentPill({
  label,
  versionNumber,
  className,
}: {
  label: string;
  versionNumber: number | undefined;
  className: string;
}) {
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-semibold",
        className,
      )}
    >
      <span className="h-1.5 w-1.5 rounded-full bg-current" aria-hidden />
      {label} {versionNumber !== undefined ? `V${versionNumber}` : "—"}
    </span>
  );
}

export function DeploymentStatusRow({
  stagingVersionId,
  liveVersionId,
  versionNumberById,
}: DeploymentStatusRowProps) {
  if (!stagingVersionId && !liveVersionId) return null;

  return (
    <div className="flex flex-wrap items-center gap-3" data-testid="deployment-status-row">
      {stagingVersionId && (
        <DeploymentPill
          label="STAGING"
          versionNumber={versionNumberById(stagingVersionId)}
          className="bg-status-stagingBg text-status-stagingFg"
        />
      )}
      {liveVersionId && (
        <DeploymentPill
          label="LIVE"
          versionNumber={versionNumberById(liveVersionId)}
          className="bg-status-liveBg text-status-liveFg"
        />
      )}
    </div>
  );
}
