"use client";

// PreviewModal (FRONTEND CONTRACT §2.5, Task 15).
//
// Placeholder "Preview" modal opened from the page-title Preview button. Real
// rendered preview is post-MVP; for now it shows a "Preview coming soon"
// message. Radix Dialog gives focus trap + Escape close.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";

export function PreviewModal() {
  const [open, setOpen] = useState(false);

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <button
          type="button"
          className="inline-flex items-center gap-2 rounded-lg border border-status-prevBg bg-bg-elevated px-4 py-2 text-sm font-medium text-nav transition-colors hover:border-brand-400 hover:bg-brand-50 hover:text-brand-700"
        >
          Preview
        </button>
      </Dialog.Trigger>

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            Preview
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-status-prevFg">
            Preview coming soon. Live outcome previews will render here in a
            future release.
          </Dialog.Description>
          <div className="mt-6 flex justify-end">
            <Dialog.Close asChild>
              <button
                type="button"
                className="rounded-lg bg-brand-600 px-4 py-2 text-sm font-medium text-white hover:bg-brand-700"
              >
                Close
              </button>
            </Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
