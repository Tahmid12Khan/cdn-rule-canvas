// Component editor route (design §5.1 / §5.3). Server shell: reads the slug
// param and renders the client orchestrator, which resolves the slug → id and
// loads the component + versions through TanStack Query.
import { ComponentEditorPage } from "@/components/component-library/ComponentEditorPage";

export default async function ComponentEditorRoute({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  return <ComponentEditorPage slug={slug} />;
}
