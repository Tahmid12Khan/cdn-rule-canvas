import { ComponentLibraryClient } from "@/components/component-library/ComponentLibraryClient";
import { listComponentTemplates } from "@/lib/api/componentTemplates";

const PAGE_SIZE = 20;

// Component library list (design §5.1). Server Component shell. SSR-prefetches
// the first page and seeds it into the client list as initialData so there's no
// hydrate→fetch waterfall. Best-effort: on failure the client re-fetches and
// surfaces its own ErrorBanner. Mirrors app/products/sites/page.tsx.
export default async function ComponentsPage() {
  const initialComponents = await listComponentTemplates({
    page: 1,
    page_size: PAGE_SIZE,
  }).catch(() => undefined);

  return <ComponentLibraryClient initialComponents={initialComponents} />;
}
