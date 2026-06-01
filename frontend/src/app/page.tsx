import Link from "next/link";

import { BackendStatusCard } from "@/components/BackendStatusCard";
import { BrandHeader } from "@/components/BrandHeader";
import { WelcomePanel } from "@/components/onboarding/WelcomePanel";

// Server component: composes the static landing page. Interactivity lives in
// the leaf client components <BackendStatusCard/> and <WelcomePanel/>.
export default function HomePage() {
  return (
    <div className="flex flex-col gap-8">
      <BrandHeader />
      <WelcomePanel />
      <BackendStatusCard />
      <Link
        href="/products/features"
        className="inline-flex w-fit items-center rounded-md bg-action-600 px-4 py-2 text-sm font-medium text-white hover:bg-action-700"
      >
        Browse Features
      </Link>
    </div>
  );
}
