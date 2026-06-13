"use client";

// Version bar (design §5.3). Drives version switching, the default pointer
// (Track latest vs Pin to a specific version), version creation (modal) and
// deletion (guarded so the last version can't be removed). The HTML/variables
// editing of the *selected* version lives in the editor body; this bar owns
// version-level concerns.
import { useState } from "react";

import { CreateVersionModal } from "@/components/component-library/CreateVersionModal";
import { ApiError } from "@/lib/api/client";
import {
  useDeleteVersion,
  useMakeDefaultVersion,
  useUpdateComponentTemplate,
} from "@/lib/api/componentTemplates";
import type {
  ComponentTemplateRead,
  DefaultMode,
} from "@/lib/schemas/componentTemplates";

const selectClass =
  "rounded-md border border-border bg-bg px-2.5 py-1.5 text-sm text-fg shadow-sm focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent";

interface VersionBarProps {
  component: ComponentTemplateRead;
  selectedVnum: number;
  onSelectVersion: (vnum: number) => void;
  // True while the editor body has unsaved edits to the selected version.
  dirty?: boolean;
}

export function VersionBar({
  component,
  selectedVnum,
  onSelectVersion,
  dirty = false,
}: VersionBarProps) {
  const [createOpen, setCreateOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const versions = [...component.versions].sort(
    (a, b) => b.version_number - a.version_number,
  );
  const isLastVersion = versions.length <= 1;

  const makeDefault = useMakeDefaultVersion(component.id);
  const deleteVersion = useDeleteVersion(component.id);
  const updateComponent = useUpdateComponentTemplate(component.id);

  function reportError(err: unknown, fallback: string) {
    setError(err instanceof ApiError ? err.message : fallback);
  }

  function handleModeChange(mode: DefaultMode) {
    setError(null);
    if (mode === "latest") {
      updateComponent.mutate(
        { default_mode: "latest" },
        { onError: (e) => reportError(e, "Could not switch to track-latest") },
      );
    } else {
      // Pin to the currently selected version.
      updateComponent.mutate(
        { default_mode: "pinned", default_version_number: selectedVnum },
        { onError: (e) => reportError(e, "Could not pin the default version") },
      );
    }
  }

  function handlePinSelected() {
    setError(null);
    makeDefault.mutate(selectedVnum, {
      onError: (e) => reportError(e, "Could not set the default version"),
    });
  }

  function handleDelete() {
    setError(null);
    if (isLastVersion) {
      setError("A component must keep at least one version.");
      return;
    }
    deleteVersion.mutate(selectedVnum, {
      onSuccess: () => {
        // Re-point the editor at the newest remaining version.
        const remaining = versions
          .filter((v) => v.version_number !== selectedVnum)
          .map((v) => v.version_number);
        if (remaining.length > 0) onSelectVersion(Math.max(...remaining));
      },
      onError: (e) => reportError(e, "Could not delete this version"),
    });
  }

  const selectedIsDefault =
    component.default_version_number === selectedVnum;
  const busy =
    makeDefault.isPending ||
    deleteVersion.isPending ||
    updateComponent.isPending;

  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border bg-bg-elevated p-3">
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2 text-sm text-fg">
          Version
          <select
            value={selectedVnum}
            onChange={(e) => onSelectVersion(Number(e.target.value))}
            className={selectClass}
            aria-label="Select version"
          >
            {versions.map((v) => (
              <option key={v.version_number} value={v.version_number}>
                v{v.version_number}
                {v.is_default ? " (default)" : ""}
              </option>
            ))}
          </select>
        </label>

        {dirty && (
          <span className="text-xs font-medium text-accent-onMuted">
            Unsaved changes
          </span>
        )}

        <div className="ml-auto flex items-center gap-2">
          <button
            type="button"
            onClick={() => setCreateOpen(true)}
            className="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-fg transition hover:bg-bg-overlay"
          >
            + New version
          </button>
          <button
            type="button"
            onClick={handleDelete}
            disabled={isLastVersion || busy}
            title={
              isLastVersion
                ? "A component must keep at least one version"
                : undefined
            }
            className="rounded-md border border-border px-3 py-1.5 text-xs font-medium text-danger transition hover:bg-danger/10 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Delete version
          </button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-3 border-t border-border pt-2">
        <span className="text-xs font-medium text-fg-muted">Default</span>
        <label className="flex items-center gap-1.5 text-sm text-fg">
          <input
            type="radio"
            name="default-mode"
            checked={component.default_mode === "latest"}
            onChange={() => handleModeChange("latest")}
            disabled={busy}
            className="h-3.5 w-3.5 text-accent focus:ring-accent"
          />
          Track latest
        </label>
        <label className="flex items-center gap-1.5 text-sm text-fg">
          <input
            type="radio"
            name="default-mode"
            checked={component.default_mode === "pinned"}
            onChange={() => handleModeChange("pinned")}
            disabled={busy}
            className="h-3.5 w-3.5 text-accent focus:ring-accent"
          />
          Pin to a version
        </label>

        {component.default_mode === "pinned" && (
          <button
            type="button"
            onClick={handlePinSelected}
            disabled={selectedIsDefault || busy}
            className="rounded-md border border-border px-3 py-1 text-xs font-medium text-fg transition hover:bg-bg-overlay disabled:cursor-not-allowed disabled:opacity-50"
          >
            {selectedIsDefault
              ? `v${selectedVnum} is the default`
              : `Pin default to v${selectedVnum}`}
          </button>
        )}
      </div>

      {error && (
        <p role="alert" className="text-xs font-medium text-danger">
          {error}
        </p>
      )}

      <CreateVersionModal
        componentId={component.id}
        open={createOpen}
        onOpenChange={setCreateOpen}
        onCreated={(vnum) => onSelectVersion(vnum)}
      />
    </div>
  );
}
