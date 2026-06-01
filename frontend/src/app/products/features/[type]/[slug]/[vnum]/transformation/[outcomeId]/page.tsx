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

export default function OutcomeEditorRoute({
  params,
}: {
  params: { type: string; slug: string; vnum: string; outcomeId: string };
}) {
  return (
    <OutcomeEditorPage
      featureType={params.type}
      featureSlug={params.slug}
      vnum={params.vnum}
      outcomeId={params.outcomeId}
    />
  );
}
