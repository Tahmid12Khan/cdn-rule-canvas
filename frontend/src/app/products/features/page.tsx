import { FeaturesListClient } from "@/components/features/FeaturesListClient";
import { listFeatures } from "@/lib/api/features";

const PAGE_SIZE = 20;

// Features list (FRONTEND CONTRACT §2.2, Task 06). Server Component shell. It
// SSR-prefetches the first features page and seeds it into the client
// list as initialData so there's no hydrate→fetch waterfall. Best-effort: on
// failure the client re-fetches and surfaces its own ErrorBanner.
export default async function FeaturesPage() {
  const initialFeatures = await listFeatures({
    page: 1,
    page_size: PAGE_SIZE,
  }).catch(() => undefined);

  return <FeaturesListClient initialFeatures={initialFeatures} />;
}
