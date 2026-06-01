"use client";

// Inline-editable version description with a pencil icon (Task 11). PATCHes on
// blur / Enter, optimistic on the ['version', fid, vnum] cache. Escape cancels.
import { useEffect, useRef, useState } from "react";
import {
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";

import { updateVersion, type VersionRead } from "@/lib/api/canvasVersions";

interface DescriptionEditableProps {
  fid: string;
  vnum: number;
  description: string | null;
  // Editing only permitted on DRAFT versions.
  editable: boolean;
}

export function DescriptionEditable({
  fid,
  vnum,
  description,
  editable,
}: DescriptionEditableProps) {
  const queryClient = useQueryClient();
  const queryKey = ["version", fid, vnum] as const;
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(description ?? "");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setValue(description ?? "");
  }, [description]);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  const mutation = useMutation({
    mutationFn: (next: string) =>
      updateVersion(fid, vnum, { description: next }),
    onMutate: async (next) => {
      await queryClient.cancelQueries({ queryKey });
      const previous = queryClient.getQueryData<VersionRead>(queryKey);
      if (previous) {
        queryClient.setQueryData<VersionRead>(queryKey, {
          ...previous,
          description: next,
        });
      }
      return { previous };
    },
    onError: (_err, _next, ctx) => {
      if (ctx?.previous) queryClient.setQueryData(queryKey, ctx.previous);
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey });
    },
  });

  function commit() {
    setEditing(false);
    const trimmed = value.trim();
    if (trimmed !== (description ?? "")) mutation.mutate(trimmed);
  }

  function cancel() {
    setValue(description ?? "");
    setEditing(false);
  }

  if (!editing) {
    return (
      <div className="flex items-center gap-2">
        <p className="text-sm text-status-prevFg">
          {description?.trim() ? description : "No description"}
        </p>
        {editable && (
          <button
            type="button"
            aria-label="Edit description"
            onClick={() => setEditing(true)}
            className="rounded p-1 text-status-prev hover:text-brand-600"
          >
            <svg
              aria-hidden
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="h-4 w-4"
            >
              <path
                d="M12 20h9M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
        )}
      </div>
    );
  }

  return (
    <input
      ref={inputRef}
      type="text"
      value={value}
      aria-label="Version description"
      onChange={(e) => setValue(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
        else if (e.key === "Escape") cancel();
      }}
      className="w-full max-w-lg rounded-md border border-brand-500 px-3 py-1.5 text-sm focus:outline-none"
    />
  );
}
