// Outcomes library editor route (Outcomes Library design). Server shell: reads
// the slug param and renders the client orchestrator, which resolves the
// slug -> id (no by-slug backend route — fetch-then-filter the list) and loads
// the saved outcome through TanStack Query.
import { SavedOutcomeEditorPage } from "@/components/outcomes-library/SavedOutcomeEditorPage";

export default async function SavedOutcomeEditorRoute({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  return <SavedOutcomeEditorPage slug={slug} />;
}
