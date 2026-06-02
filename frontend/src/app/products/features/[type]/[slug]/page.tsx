// Version list (FRONTEND CONTRACT §2.3, Task 08). Server shell: composes the
// interactive <VersionListClient/>, which owns the feature + versions TanStack
// queries, search, pagination, and row mutations.
//
// Route params: [type] = feature type (html|json, decorative in URL),
// [slug] = feature id (the slug PK / fid).
import { VersionListClient } from "@/components/versions/VersionListClient";
import { getFeature } from "@/lib/api/features";
import { listVersions } from "@/lib/api/versions";

const PAGE_SIZE = 20;

export default async function VersionListPage({
  params,
}: {
  params: Promise<{ type: string; slug: string }>;
}) {
  const { type, slug } = await params;

  // SSR prefetch: seed the feature + first versions page so the client
  // queries hydrate with initialData (no hydrate→fetch waterfall). Best-effort:
  // on failure the client re-fetches and surfaces its own ErrorBanner.
  const [initialFeature, initialVersions] = await Promise.all([
    getFeature(slug).catch(() => undefined),
    listVersions(slug, { search: "", page: 1, page_size: PAGE_SIZE }).catch(
      () => undefined,
    ),
  ]);

  return (
    <VersionListClient
      featureId={slug}
      featureType={type}
      initialFeature={initialFeature}
      initialVersions={initialVersions}
    />
  );
}
