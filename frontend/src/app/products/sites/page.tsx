import { SitesListClient } from "@/components/sites/SitesListClient";
import { listSites } from "@/lib/api/sites";

const PAGE_SIZE = 20;

// Sites list (spec §6). Server Component shell. It SSR-prefetches the first
// sites page and seeds it into the client list as initialData so there's no
// hydrate→fetch waterfall. Best-effort: on failure the client re-fetches and
// surfaces its own ErrorBanner.
export default async function SitesPage() {
  const initialSites = await listSites({
    page: 1,
    page_size: PAGE_SIZE,
  }).catch(() => undefined);

  return <SitesListClient initialSites={initialSites} />;
}
