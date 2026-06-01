import { FeaturesListClient } from "@/components/features/FeaturesListClient";

// Features list (FRONTEND CONTRACT §2.2, Task 06). Server Component shell: it
// composes the interactive client list. The client component owns the
// TanStack Query data fetching (the scaffold QueryProvider is a per-client
// cache without server dehydration, so fetching happens client-side).
export default function FeaturesPage() {
  return <FeaturesListClient />;
}
