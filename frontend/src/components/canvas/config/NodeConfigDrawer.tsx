"use client";

// Right-side drawer to configure (edit) or inspect (read-only) a node (Tasks 13
// + Phase 1 Task D). Opened when store.configNodeId is set — double-click on a
// decision node (edit), or a single click on a decision/outcome/start node
// (inspect, especially in read-only mode).
//
// EDIT mode (store.isEditing): decision nodes render editable forms + Save
// (disabled until valid) + Delete, with Esc / outside-click discard-confirm.
// VIEW mode (!isEditing): forms render disabled/readOnly, Save + Delete are
// hidden, only a Close button shows, and the dirty/discard logic is skipped.
// Outcome / start nodes have no processor form, so they always render a small
// read-only summary of their contents.
import { useEffect, useMemo, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";

import { GenericNodeForm } from "@/components/canvas/config/GenericNodeForm";
import { useNodeTypes } from "@/hooks/useNodeTypes";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { CanvasKey, ProcessorConfig, RFNode } from "@/lib/canvas/types";

interface NodeConfigDrawerProps {
  canvasKey: CanvasKey;
}

function sameProcessor(a: ProcessorConfig, b: ProcessorConfig): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function NodeConfigDrawer({ canvasKey }: NodeConfigDrawerProps) {
  const configNodeId = useRuleBuilderStore((s) => s.configNodeId);
  const nodes = useRuleBuilderStore((s) => s.canvases[canvasKey].nodes);
  const isEditing = useRuleBuilderStore((s) => s.isEditing);
  const openNodeConfig = useRuleBuilderStore((s) => s.openNodeConfig);
  const updateNodeProcessor = useRuleBuilderStore(
    (s) => s.updateNodeProcessor,
  );
  const removeNode = useRuleBuilderStore((s) => s.removeNode);
  const { specByKind } = useNodeTypes();

  const node = useMemo<RFNode | undefined>(
    () => nodes.find((n) => n.id === configNodeId),
    [nodes, configNodeId],
  );

  const isDecision = node?.type === "decisionNode";
  const isOutcome = node?.type === "outcomeNode";
  const isStart = node?.type === "startNode";
  const initial = isDecision ? node.data.processor : null;
  const spec = initial ? specByKind(initial.type) : undefined;

  // View-only when the version is not in edit mode. Decision forms still load
  // but render disabled; Save/Delete/discard are suppressed.
  const readOnly = !isEditing;

  const [draft, setDraft] = useState<ProcessorConfig | null>(initial);
  const [valid, setValid] = useState(false);
  const [confirmDiscard, setConfirmDiscard] = useState(false);

  // Re-seed draft whenever a new node is opened.
  useEffect(() => {
    setDraft(initial);
    setValid(false);
    setConfirmDiscard(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [configNodeId]);

  const dirty =
    !readOnly &&
    isDecision &&
    draft != null &&
    !sameProcessor(draft, node.data.processor);

  function close() {
    setConfirmDiscard(false);
    openNodeConfig(null);
  }

  function requestClose() {
    if (dirty) {
      setConfirmDiscard(true);
    } else {
      close();
    }
  }

  function onSave() {
    if (readOnly || !isDecision || !draft || !valid) return;
    updateNodeProcessor(canvasKey, node.id, draft);
    close();
  }

  function onDelete() {
    if (readOnly || !node) return;
    removeNode(canvasKey, node.id);
    close();
  }

  // Open for any inspectable node (decision / outcome / start).
  const open = Boolean(configNodeId) && (isDecision || isOutcome || isStart);

  const title = readOnly ? "View node" : "Edit node";
  const description = isDecision
    ? readOnly
      ? "Read-only view of how this decision evaluates incoming requests."
      : "Configure how this decision evaluates incoming requests."
    : isStart
      ? "The entry point of this rule. Evaluation begins here."
      : "Read-only details for this node.";

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) requestClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content
          // Anchor as a right-side drawer.
          className="fixed right-0 top-0 z-50 flex h-full w-full max-w-sm flex-col border-l border-status-prevBg bg-bg-elevated shadow-xl focus:outline-none"
          onEscapeKeyDown={(e) => {
            if (dirty) {
              e.preventDefault();
              setConfirmDiscard(true);
            }
          }}
          onInteractOutside={(e) => {
            if (dirty) {
              e.preventDefault();
              setConfirmDiscard(true);
            }
          }}
        >
          <header className="flex items-center justify-between border-b border-status-prevBg px-5 py-4">
            <Dialog.Title className="text-base font-semibold text-nav">
              {title}
            </Dialog.Title>
            {!readOnly && isDecision && (
              <button
                type="button"
                onClick={onDelete}
                aria-label="Delete node"
                className="rounded-md border border-danger px-2.5 py-1 text-xs font-medium text-danger hover:bg-danger-bg"
              >
                Delete node
              </button>
            )}
          </header>

          <Dialog.Description className="px-5 pt-3 text-xs text-status-prevFg">
            {description}
          </Dialog.Description>

          <div className="flex-1 overflow-y-auto px-5 py-4">
            {/* Decision: render the generic manifest-driven form (disabled in
                view mode). key on the node id so the form remounts (fresh state)
                when a different node is opened. */}
            {isDecision && initial && spec && (
              <GenericNodeForm
                key={configNodeId ?? "node"}
                spec={spec}
                initial={initial}
                disabled={readOnly}
                onChange={(d, v) => {
                  setDraft(d);
                  setValid(v);
                }}
              />
            )}
            {isDecision && initial && !spec && (
              <p className="text-sm text-danger" data-testid="unknown-kind">
                Unknown node type “{initial.type}”. This node type is not
                available in the current node-type manifest.
              </p>
            )}

            {/* Outcome: no processor form — show its label / linked outcome. */}
            {isOutcome && node.type === "outcomeNode" && (
              <dl className="space-y-3" data-testid="outcome-inspect">
                <div>
                  <dt className="mb-1 text-sm font-medium text-nav">Outcome</dt>
                  <dd className="text-sm text-status-prevFg">
                    {node.data.title || "Untitled outcome"}
                  </dd>
                </div>
                <div>
                  <dt className="mb-1 text-sm font-medium text-nav">
                    Outcome ID
                  </dt>
                  <dd className="break-all font-mono text-xs text-status-prevFg">
                    {node.data.outcomeId}
                  </dd>
                </div>
              </dl>
            )}

            {/* Start: frontend-only entry marker. */}
            {isStart && (
              <dl className="space-y-3" data-testid="start-inspect">
                <div>
                  <dt className="mb-1 text-sm font-medium text-nav">Node</dt>
                  <dd className="text-sm text-status-prevFg">Start</dd>
                </div>
                <div>
                  <dd className="text-sm text-status-prevFg">
                    The rule begins evaluating from the first decision connected
                    to this entry point.
                  </dd>
                </div>
              </dl>
            )}
          </div>

          <footer className="flex items-center justify-end gap-2 border-t border-status-prevBg px-5 py-4">
            {readOnly ? (
              <button
                type="button"
                onClick={close}
                className="rounded-md bg-brand-500 px-4 py-2 text-sm font-semibold text-white hover:bg-brand-600"
              >
                Close
              </button>
            ) : (
              <>
                <button
                  type="button"
                  onClick={requestClose}
                  className="rounded-md border border-status-prevBg px-4 py-2 text-sm font-medium text-status-prevFg hover:bg-bg-overlay"
                >
                  Cancel
                </button>
                {isDecision && (
                  <button
                    type="button"
                    onClick={onSave}
                    disabled={!valid}
                    className="rounded-md bg-brand-500 px-4 py-2 text-sm font-semibold text-white hover:bg-brand-600 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    Save
                  </button>
                )}
              </>
            )}
          </footer>

          {confirmDiscard && (
            <div className="absolute inset-0 z-10 flex items-center justify-center bg-black/60 backdrop-blur-sm p-6">
              <div
                role="alertdialog"
                aria-label="Discard changes"
                className="w-full max-w-xs rounded-lg bg-bg-elevated p-5 shadow-xl"
              >
                <h3 className="text-sm font-semibold text-nav">
                  Discard changes?
                </h3>
                <p className="mt-1 text-sm text-status-prevFg">
                  Your unsaved edits to this node will be lost.
                </p>
                <div className="mt-4 flex justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => setConfirmDiscard(false)}
                    className="rounded-md border border-status-prevBg px-3 py-1.5 text-sm font-medium text-status-prevFg hover:bg-bg-overlay"
                  >
                    Keep editing
                  </button>
                  <button
                    type="button"
                    onClick={close}
                    className="rounded-md bg-danger px-3 py-1.5 text-sm font-semibold text-white hover:opacity-90"
                  >
                    Discard
                  </button>
                </div>
              </div>
            </div>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
