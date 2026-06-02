// Edit Outcome page (FRONTEND CONTRACT §2.5, Task 15). Server shell: it only
// reads the route params and renders the client orchestrator. The actual data
// fetching (the outcome + its components) happens client-side through TanStack
// Query inside <OutcomeEditorPage/> so edits, optimistic mutations and the
// dirty/discard flow all live in one client boundary.
//
// Route params (all strings from the URL):
//   [type]      feature type segment (html | json, decorative)
//   [slug]      feature id (the slug PK, {fid})
//   [vnum]      version_number as a string (parse Number for version API calls)
//   [outcomeId] outcome UUID ({oid}) — passed straight to /outcomes/{oid}
import { OutcomeEditorPage } from "@/components/outcome/OutcomeEditorPage";
import { getVersion } from "@/lib/api/canvasVersions";
import { getOutcome } from "@/lib/api/outcomes";

export default async function OutcomeEditorRoute({
  params,
}: {
  params: Promise<{
    type: string;
    slug: string;
    vnum: string;
    outcomeId: string;
  }>;
}) {
  const { type, slug, vnum, outcomeId } = await params;

  // SSR prefetch: seed the outcome + version so the client editor
  // hydrates with initialData. Best-effort — the client re-fetches (and shows
  // its own ErrorBanner) if the backend is unreachable.
  const vnumNumber = Number(vnum);
  const [initialOutcome, initialVersion] = await Promise.all([
    getOutcome(outcomeId).catch(() => undefined),
    Number.isFinite(vnumNumber)
      ? getVersion(slug, vnumNumber).catch(() => undefined)
      : Promise.resolve(undefined),
  ]);

  return (
    <OutcomeEditorPage
      featureType={type}
      featureSlug={slug}
      vnum={vnum}
      outcomeId={outcomeId}
      initialOutcome={initialOutcome}
      initialVersion={initialVersion}
    />
  );
}
