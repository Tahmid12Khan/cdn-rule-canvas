// Version list (FRONTEND CONTRACT §2.3, Task 08). Server shell: composes the
// interactive <VersionListClient/>, which owns the feature + versions TanStack
// queries, search, pagination, and row mutations.
//
// Route params: [type] = feature type (html|json, decorative in URL),
// [slug] = feature id (the slug PK / fid).
import { VersionListClient } from "@/components/versions/VersionListClient";

export default function VersionListPage({
  params,
}: {
  params: { type: string; slug: string };
}) {
  return (
    <VersionListClient featureId={params.slug} featureType={params.type} />
  );
}
