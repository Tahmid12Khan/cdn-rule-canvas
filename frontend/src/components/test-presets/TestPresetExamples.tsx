// Server-safe presentational onboarding helper: a plain-language card list of
// canned starter test templates. Each card explains, in non-technical terms,
// WHEN you'd reach for it and WHAT it simulates, plus a one-click
// "Use this template" button that hands the full create form a ready-made
// slug / name / kind / payload so a first-time user never has to author JSON.
//
// No client hooks/state of its own — the button just calls an `onUse` callback
// the parent (TestPresetsListClient / TestPresetFormModal) provides. It still
// renders an interactive onClick, so it must live in a Client tree; both real
// consumers are "use client". It's a stateless, shared presentational helper.
import type { TestPresetKind } from "@/lib/api/test-presets";

export interface TestPresetExample {
  slug: string;
  name: string;
  kind: TestPresetKind;
  payload: Record<string, unknown>;
  // One short sentence: when would I use this?
  when: string;
  // One short sentence: what request does it simulate?
  simulates: string;
}

// The three starter templates from the spec. Kept small + obvious on purpose —
// these are teaching examples, not an exhaustive catalogue.
export const TEST_PRESET_EXAMPLES: TestPresetExample[] = [
  {
    slug: "mobile-paywall-test",
    name: "Mobile paywall test",
    kind: "rule",
    payload: { device_type: "mobile", meta_tags: { subscription: "false" } },
    when: "You want to check that a visitor on a phone who has NOT subscribed gets shown the paywall.",
    simulates:
      "A mobile device with a “subscription: false” page tag — the classic non-subscriber on mobile.",
  },
  {
    slug: "desktop-premium-reader",
    name: "Desktop premium reader",
    kind: "rule",
    payload: { device_type: "desktop", meta_tags: { subscription: "true" } },
    when: "You want to confirm a paying reader on a computer is let through untouched.",
    simulates:
      "A desktop device with a “subscription: true” page tag — a logged-in subscriber.",
  },
  {
    slug: "live-url-check",
    name: "Live URL check",
    kind: "url",
    payload: { url: "https://example.com/article/123", headers: {} },
    when: "You want to test your rule against a real page on a configured site instead of made-up inputs.",
    simulates:
      "Fetching a real article URL through the proxy and running the rule on the actual response.",
  },
];

const kindLabel: Record<TestPresetKind, string> = {
  rule: "Synthetic inputs",
  url: "Live page",
};

interface TestPresetExamplesProps {
  // Called with a chosen example when the user clicks "Use this template". The
  // parent pre-fills the create form from it.
  onUse: (example: TestPresetExample) => void;
  // Optional heading override (the modal uses a quieter helper tone).
  heading?: string;
  description?: string;
}

export function TestPresetExamples({
  onUse,
  heading = "Start from an example",
  description = "New to test templates? Pick a starter below — it fills in everything for you, and you can rename or tweak it before saving.",
}: TestPresetExamplesProps) {
  return (
    <section aria-label="Starter test templates" className="flex flex-col gap-3">
      <div>
        <h2 className="text-sm font-semibold text-nav">{heading}</h2>
        <p className="mt-0.5 text-sm text-status-prevFg">{description}</p>
      </div>
      <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {TEST_PRESET_EXAMPLES.map((example) => (
          <li
            key={example.slug}
            className="flex flex-col gap-2 rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-sm"
          >
            <div className="flex items-center justify-between gap-2">
              <h3 className="text-sm font-semibold text-nav">{example.name}</h3>
              <span className="shrink-0 rounded-full border border-status-prevBg px-2 py-0.5 text-[11px] font-medium text-status-prevFg">
                {kindLabel[example.kind]}
              </span>
            </div>
            <dl className="flex flex-col gap-1.5 text-xs text-status-prevFg">
              <div>
                <dt className="font-semibold text-nav">When to use it</dt>
                <dd className="mt-0.5">{example.when}</dd>
              </div>
              <div>
                <dt className="font-semibold text-nav">What it simulates</dt>
                <dd className="mt-0.5">{example.simulates}</dd>
              </div>
            </dl>
            <button
              type="button"
              onClick={() => onUse(example)}
              className="mt-1 w-fit rounded-md bg-action-600 px-3 py-1.5 text-xs font-medium text-white shadow-sm transition hover:bg-action-700"
            >
              Use this template
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
