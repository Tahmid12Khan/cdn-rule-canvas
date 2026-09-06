import { ProductCatalogueClient } from "@/components/product-catalogue/ProductCatalogueClient";
import { listProducts } from "@/lib/api/products";

const PAGE_SIZE = 20;

// Product Catalogue list. Server Component shell. It SSR-prefetches the first
// products page and seeds it into the client list as initialData so there's no
// hydrate→fetch waterfall. Best-effort: on failure the client re-fetches and
// surfaces its own ErrorBanner.
export default async function ProductCataloguePage() {
  const initialProducts = await listProducts({
    page: 1,
    page_size: PAGE_SIZE,
  }).catch(() => undefined);

  return <ProductCatalogueClient initialProducts={initialProducts} />;
}
