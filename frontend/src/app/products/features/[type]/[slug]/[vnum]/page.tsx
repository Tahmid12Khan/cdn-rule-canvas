// Rule Builder / Version detail page (FRONTEND CONTRACT §2.4, Tasks 11–14).
// Server shell: fetches the version (by vnum) for SSR/initial data, then hands
// off to the client RuleBuilderClient which owns the canvas working state.
//
// Route params: [type] = feature type (html|json, decorative), [slug] = fid,
// [vnum] = version_number as a string (parse Number(params.vnum) for API calls —
// version endpoints key on vnum, not UUID).
import { notFound } from "next/navigation";

import { RuleBuilderClient } from "@/components/version/RuleBuilderClient";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { getVersion } from "@/lib/api/canvasVersions";

export default async function VersionDetailPage({
  params,
}: {
  params: { type: string; slug: string; vnum: string };
}) {
  const vnum = Number(params.vnum);
  if (!Number.isInteger(vnum)) notFound();

  try {
    const version = await getVersion(params.slug, vnum);
    return (
      <RuleBuilderClient
        fid={params.slug}
        vnum={vnum}
        type={params.type}
        initialVersion={version}
      />
    );
  } catch {
    // SSR fetch failed (backend unreachable / not found). Render a recoverable
    // banner rather than crashing the route; the client re-fetches via Query.
    return (
      <div className="px-6 py-6">
        <ErrorBanner message="Could not load this version. Is the backend running?" />
      </div>
    );
  }
}
