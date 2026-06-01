"use client";

// ApplicabilityForm (spec §4 req 8). Version-level gate that controls WHEN a
// feature's rules apply:
//   - HTML features: a CSS selector that must match ≥1 element.
//   - JSON features: a JSONPath that must match ≥1 node.
// Empty = "always apply" (whenever the response content-type matches). Edited
// only on a DRAFT version (read-only otherwise, mirroring the rule_graph lock);
// persisted via patchApplicability. Seeded from version.applicability.
import { useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { ApiError } from "@/lib/api/client";
import {
  patchApplicability,
  type VersionRead,
} from "@/lib/api/canvasVersions";
import type { Applicability } from "@/lib/api/ruleGraph";
import { toUserError, type UserError } from "@/lib/errors/userError";

interface ApplicabilityFormProps {
  fid: string;
  vnum: number;
  // Seed value from the version (defaults to "always apply" when absent).
  applicability: Applicability | undefined;
  featureType: "html" | "json";
  editable: boolean;
}

export function ApplicabilityForm({
  fid,
  vnum,
  applicability,
  featureType,
  editable,
}: ApplicabilityFormProps) {
  const queryClient = useQueryClient();
  const isJson = featureType === "json";

  const seeded = isJson
    ? (applicability?.json_selector ?? "")
    : (applicability?.html_selector ?? "");
  // Track the last-saved value so the Save button can disable when unchanged.
  const savedRef = useRef(seeded);
  const [value, setValue] = useState(seeded);
  const [error, setError] = useState<UserError | null>(null);
  const [rawResponse, setRawResponse] = useState<string | undefined>(undefined);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const save = useMutation({
    mutationFn: () => {
      const trimmed = value.trim();
      const body: Applicability = isJson
        ? { json_selector: trimmed || null }
        : { html_selector: trimmed || null };
      return patchApplicability(fid, vnum, body);
    },
    onSuccess: (updated: VersionRead) => {
      setError(null);
      setRawResponse(undefined);
      savedRef.current = value;
      setSavedAt(Date.now());
      queryClient.setQueryData(["version", fid, vnum], updated);
    },
    onError: (err) => {
      setRawResponse(err instanceof ApiError ? err.rawBody : undefined);
      setError(toUserError(err, { surface: "save" }));
    },
  });

  const label = isJson ? "JSONPath" : "CSS selector";
  const placeholder = isJson
    ? "$.books[?(@.title contains 'Century')]"
    : "#article, .article-body";
  const dirty = value !== savedRef.current;

  return (
    <section
      aria-label="Applicability gate"
      className="rounded-lg border border-status-prevBg bg-bg-elevated p-4 shadow-sm"
    >
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-nav">When to apply</h3>
        {savedAt && !dirty && (
          <span className="font-mono text-xs text-fg-muted">Saved</span>
        )}
      </div>
      <p className="mt-1 text-xs text-status-prevFg">
        {isJson
          ? "Apply these rules only when the JSON response matches this JSONPath."
          : "Apply these rules only when the HTML response matches this CSS selector."}{" "}
        Leave empty to always apply.
      </p>

      <label className="mt-3 flex flex-col gap-1 text-xs font-medium text-nav">
        {label}
        <input
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder={placeholder}
          disabled={!editable}
          aria-label={label}
          className="rounded-md border border-status-prevBg px-2 py-1.5 font-mono text-sm focus:border-brand-500 focus:outline-none disabled:cursor-not-allowed disabled:opacity-60"
        />
      </label>

      {editable && (
        <div className="mt-3">
          <button
            type="button"
            onClick={() => save.mutate()}
            disabled={save.isPending || !dirty}
            className="rounded-md bg-action px-3 py-1.5 text-sm font-semibold text-white hover:bg-action-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {save.isPending ? "Saving…" : "Save"}
          </button>
        </div>
      )}

      {error && (
        <div className="mt-3">
          <ErrorBanner error={error} rawResponse={rawResponse} />
        </div>
      )}
    </section>
  );
}
