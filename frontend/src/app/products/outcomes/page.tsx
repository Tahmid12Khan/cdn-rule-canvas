import { SavedOutcomeLibraryClient } from "@/components/outcomes-library/SavedOutcomeLibraryClient";
import { listSavedOutcomes } from "@/lib/api/savedOutcomes";

const PAGE_SIZE = 20;

// Outcomes library list (Outcomes Library design). Server Component shell.
// SSR-prefetches the first page and seeds it into the client list as
// initialData so there's no hydrate→fetch waterfall. Best-effort: on failure
// the client re-fetches and surfaces its own ErrorBanner. Mirrors
// app/products/components/page.tsx.
export default async function OutcomesPage() {
  const initialOutcomes = await listSavedOutcomes({
    page: 1,
    page_size: PAGE_SIZE,
  }).catch(() => undefined);

  return <SavedOutcomeLibraryClient initialOutcomes={initialOutcomes} />;
}
