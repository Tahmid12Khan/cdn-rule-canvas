// Version detail header: "Version {N}" + deployment status pills (Task 11).
// Presentational (server-safe). The version's own status pill plus derived
// LIVE/STAGING deployment chips when this version is the feature's live/staging
// version.
import { StatusPill } from "@/components/ui/StatusPill";
import type { VersionStatus } from "@/lib/api/enums";

interface VersionHeaderProps {
  versionNumber: number;
  status: VersionStatus;
  isLive: boolean;
  isStaging: boolean;
}

export function VersionHeader({
  versionNumber,
  status,
  isLive,
  isStaging,
}: VersionHeaderProps) {
  return (
    <div className="flex flex-wrap items-center gap-3">
      <h1 className="text-2xl font-bold text-nav">Version {versionNumber}</h1>
      <div className="flex items-center gap-2">
        <StatusPill status={status} />
        {isStaging && status !== "staging" && <StatusPill status="staging" />}
        {isLive && status !== "live" && <StatusPill status="live" />}
      </div>
    </div>
  );
}
