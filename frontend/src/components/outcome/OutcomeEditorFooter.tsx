"use client";

// OutcomeEditorFooter (FRONTEND CONTRACT §2.5, Task 15).
//
// Sticky footer for the outcome editor. Cancel returns to the version page; if
// there are unsaved changes it first opens a discard-confirm dialog (Radix).
// Save is disabled while invalid or while a save is in flight, and shows a
// saving spinner label. The parent owns the actual save/navigation logic.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";

interface OutcomeEditorFooterProps {
  dirty: boolean;
  canSave: boolean;
  saving: boolean;
  /** Hide the Save button entirely for read-only (non-draft) versions. */
  showSave?: boolean;
  onCancel: () => void;
  onSave: () => void;
}

export function OutcomeEditorFooter({
  dirty,
  canSave,
  saving,
  showSave = true,
  onCancel,
  onSave,
}: OutcomeEditorFooterProps) {
  const [confirmOpen, setConfirmOpen] = useState(false);

  function handleCancelClick() {
    if (dirty) {
      setConfirmOpen(true);
    } else {
      onCancel();
    }
  }

  return (
    <div className="sticky bottom-0 z-20 flex items-center justify-end gap-3 border-t border-status-prevBg bg-bg-elevated px-6 py-4">
      <Dialog.Root open={confirmOpen} onOpenChange={setConfirmOpen}>
        <button
          type="button"
          onClick={handleCancelClick}
          disabled={saving}
          className="rounded-lg border border-status-prevBg bg-bg-elevated px-4 py-2 text-sm font-medium text-nav transition-colors hover:bg-status-prevBg/40 disabled:opacity-50"
        >
          Cancel
        </button>

        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-xl bg-bg-elevated p-6 shadow-xl focus:outline-none">
            <Dialog.Title className="text-lg font-semibold text-nav">
              Discard changes?
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-status-prevFg">
              You have unsaved changes. Leaving now will discard them.
            </Dialog.Description>
            <div className="mt-6 flex justify-end gap-3">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="rounded-lg border border-status-prevBg bg-bg-elevated px-4 py-2 text-sm font-medium text-nav hover:bg-status-prevBg/40"
                >
                  Keep editing
                </button>
              </Dialog.Close>
              <button
                type="button"
                onClick={() => {
                  setConfirmOpen(false);
                  onCancel();
                }}
                className="rounded-lg bg-danger px-4 py-2 text-sm font-medium text-white hover:opacity-90"
              >
                Discard
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {showSave && (
        <button
          type="button"
          onClick={onSave}
          disabled={!canSave || saving}
          className="rounded-lg bg-brand-600 px-5 py-2 text-sm font-semibold text-white transition-colors hover:bg-brand-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {saving ? "Saving…" : "Save"}
        </button>
      )}
    </div>
  );
}
