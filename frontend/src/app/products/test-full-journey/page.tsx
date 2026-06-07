import { FullJourneyClient } from "@/components/full-journey/FullJourneyClient";

// Test Full Journey (spec item 5). Server Component shell. The interactive form
// (URL + headers + env + per-feature version overrides) and the results live in
// the client island, which fetches the feature/version lists on mount and posts
// to the proxy's full-journey endpoint. Mirrors app/products/sites/page.tsx.
export default function TestFullJourneyPage() {
  return <FullJourneyClient />;
}
