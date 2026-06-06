import { TestPresetsListClient } from "@/components/test-presets/TestPresetsListClient";
import { listTestPresets } from "@/lib/api/test-presets";

const PAGE_SIZE = 20;

// Test-presets list. Server Component shell. SSR-prefetches the first page and
// seeds it into the client list as initialData so there's no hydrate→fetch
// waterfall. Best-effort: on failure the client re-fetches and surfaces its own
// ErrorBanner. Mirrors app/products/sites/page.tsx.
export default async function TestPresetsPage() {
  const initialPresets = await listTestPresets({
    page: 1,
    page_size: PAGE_SIZE,
  }).catch(() => undefined);

  return <TestPresetsListClient initialPresets={initialPresets} />;
}
