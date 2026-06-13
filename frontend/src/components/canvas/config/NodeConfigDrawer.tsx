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
import { useQuery } from "@tanstack/react-query";

import {
  GenericNodeForm,
  type OutcomeSelectOption,
} from "@/components/canvas/config/GenericNodeForm";
import { useNodeTypes } from "@/hooks/useNodeTypes";
import {
  componentKeys,
  listComponentTemplates,
} from "@/lib/api/componentTemplates";
import { validateCustomLabel } from "@/lib/canvas/customLabel";
import { useRuleBuilderStore } from "@/state/ruleBuilderStore";
import type { CanvasKey, ProcessorConfig, RFNode } from "@/lib/canvas/types";

// Action kinds whose chosen component's name is cached for canvas/journey
// display (mirrors apply_outcome's outcomeTitle).
const COMPONENT_KINDS = new Set(["apply_component", "apply_component_json"]);

interface NodeConfigDrawerProps {
  canvasKey: CanvasKey;
  // The version's outcomes — options for an apply_outcome expression node's
  // outcome_select dropdown (expression-nodes-spec §2).
  outcomes?: OutcomeSelectOption[];
}

function sameProcessor(a: ProcessorConfig, b: ProcessorConfig): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function NodeConfigDrawer({
  canvasKey,
  outcomes = [],
}: NodeConfigDrawerProps) {
  const configNodeId = useRuleBuilderStore((s) => s.configNodeId);
  const nodes = useRuleBuilderStore((s) => s.canvases[canvasKey].nodes);
  const isEditing = useRuleBuilderStore((s) => s.isEditing);
  const openNodeConfig = useRuleBuilderStore((s) => s.openNodeConfig);
  const updateNodeProcessor = useRuleBuilderStore(
    (s) => s.updateNodeProcessor,
  );
  const updateNodeAction = useRuleBuilderStore((s) => s.updateNodeAction);
  const removeNode = useRuleBuilderStore((s) => s.removeNode);
  const { specByKind } = useNodeTypes();

  const node = useMemo<RFNode | undefined>(
    () => nodes.find((n) => n.id === configNodeId),
    [nodes, configNodeId],
  );

  // Component library (shares GenericNodeForm's query key/cache) so on save we
  // can denormalize the chosen component's NAME for canvas/journey display
  // (mirrors apply_outcome's outcomeTitle). Only fetched when a component node
  // is open in edit mode.
  const isComponentAction =
    node?.type === "expressionNode" &&
    COMPONENT_KINDS.has(node.data.action.type);
  const componentsQuery = useQuery({
    queryKey: componentKeys.list({ page: 1, page_size: 100 }),
    queryFn: () => listComponentTemplates({ page: 1, page_size: 100 }),
    enabled: isComponentAction && isEditing,
  });
  const componentNameById = useMemo(() => {
    const map = new Map(
      (componentsQuery.data?.items ?? []).map((c) => [c.id, c.name]),
    );
    return (id: unknown) =>
      typeof id === "string" ? map.get(id) : undefined;
  }, [componentsQuery.data]);

  const isDecision = node?.type === "decisionNode";
  const isExpression = node?.type === "expressionNode";
  const isStart = node?.type === "startNode";
  const isEnd = node?.type === "endNode";
  // Both decision (processor) and expression (action) use the generic
  // manifest-driven form, keyed off the same `{ type, …fields }` config.
  const hasForm = isDecision || isExpression;
  const initial: ProcessorConfig | null = isDecision
    ? node.data.processor
    : isExpression
      ? node.data.action
      : null;
  const spec = initial ? specByKind(initial.type) : undefined;
  // Expression nodes carry an optional snake_case custom name (spec §v2.3).
  const initialCustomLabel = isExpression ? (node.data.custom_label ?? "") : "";

  // View-only when the version is not in edit mode. Decision forms still load
  // but render disabled; Save/Delete/discard are suppressed.
  const readOnly = !isEditing;

  const [draft, setDraft] = useState<ProcessorConfig | null>(initial);
  const [valid, setValid] = useState(false);
  const [customLabel, setCustomLabel] = useState(initialCustomLabel);
  const [confirmDiscard, setConfirmDiscard] = useState(false);

  // Re-seed draft whenever a new node is opened.
  useEffect(() => {
    setDraft(initial);
    setValid(false);
    setCustomLabel(initialCustomLabel);
    setConfirmDiscard(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [configNodeId]);

  // Inline error for the custom-name field (null = valid; empty counts valid).
  const customLabelError = isExpression
    ? validateCustomLabel(customLabel)
    : null;

  const dirty =
    !readOnly &&
    hasForm &&
    initial != null &&
    draft != null &&
    (!sameProcessor(draft, initial) ||
      (isExpression && customLabel.trim() !== initialCustomLabel.trim()));

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
    if (readOnly || !node || !draft || !valid) return;
    if (isDecision) {
      updateNodeProcessor(canvasKey, node.id, draft);
    } else if (isExpression) {
      // Block save on an invalid custom name (spec §v2.3).
      if (customLabelError) return;
      // For apply_outcome, cache the chosen outcome's title for display.
      const outcomeId = draft.outcome_id;
      const outcomeTitle =
        typeof outcomeId === "string"
          ? outcomes.find((o) => o.id === outcomeId)?.title
          : undefined;
      // For apply_component / apply_component_json, cache the component's name.
      const componentName = COMPONENT_KINDS.has(draft.type)
        ? componentNameById(draft.component_id)
        : undefined;
      updateNodeAction(
        canvasKey,
        node.id,
        draft,
        outcomeTitle,
        customLabel,
        componentName,
      );
    } else {
      return;
    }
    close();
  }

  function onDelete() {
    if (readOnly || !node) return;
    removeNode(canvasKey, node.id);
    close();
  }

  // Open for any inspectable node (start / decision / expression / end).
  const open =
    Boolean(configNodeId) && (hasForm || isStart || isEnd);

  const title = readOnly ? "View node" : "Edit node";
  const description = isDecision
    ? readOnly
      ? "Read-only view of how this decision evaluates incoming requests."
      : "Configure how this decision evaluates incoming requests."
    : isExpression
      ? readOnly
        ? "Read-only view of the action this node applies to the body."
        : "Configure the action this node applies to the response body."
      : isStart
        ? "The entry point of this rule. Evaluation begins here."
        : isEnd
          ? "The terminal of this rule. The flow stops here."
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
            {!readOnly && hasForm && (
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
            {/* Decision (processor) / Expression (action): the SAME generic
                manifest-driven form (disabled in view mode). key on the node id
                so the form remounts (fresh state) when a different node opens. */}
            {hasForm && initial && spec && (
              <GenericNodeForm
                key={configNodeId ?? "node"}
                spec={spec}
                initial={initial}
                disabled={readOnly}
                outcomes={outcomes}
                onChange={(d, v) => {
                  setDraft(d);
                  setValid(v);
                }}
              />
            )}
            {hasForm && initial && !spec && (
              <p className="text-sm text-danger" data-testid="unknown-kind">
                Unknown node type “{initial.type}”. This node type is not
                available in the current node-type manifest.
              </p>
            )}

            {/* Expression nodes: optional snake_case custom name (spec §v2.3). */}
            {isExpression && initial && spec && (
              <div className="mt-4">
                <label
                  htmlFor="field-custom_label"
                  className="mb-1 block text-sm font-medium text-nav"
                >
                  Custom name (optional)
                </label>
                <input
                  id="field-custom_label"
                  type="text"
                  value={customLabel}
                  onChange={(e) => setCustomLabel(e.target.value)}
                  readOnly={readOnly}
                  disabled={readOnly}
                  placeholder="e.g. show_paywall"
                  aria-invalid={Boolean(customLabelError) || undefined}
                  className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
                />
                {customLabelError && (
                  <p className="mt-1 text-xs text-danger">{customLabelError}</p>
                )}
              </div>
            )}

            {/* Start: persisted entry marker. */}
            {isStart && (
              <dl className="space-y-3" data-testid="start-inspect">
                <div>
                  <dt className="mb-1 text-sm font-medium text-nav">Node</dt>
                  <dd className="text-sm text-status-prevFg">Start</dd>
                </div>
                <div>
                  <dd className="text-sm text-status-prevFg">
                    The rule begins evaluating from the first node connected to
                    this entry point.
                  </dd>
                </div>
              </dl>
            )}

            {/* End: terminal node. */}
            {isEnd && (
              <dl className="space-y-3" data-testid="end-inspect">
                <div>
                  <dt className="mb-1 text-sm font-medium text-nav">Node</dt>
                  <dd className="text-sm text-status-prevFg">END</dd>
                </div>
                <div>
                  <dd className="text-sm text-status-prevFg">
                    The flow stops here. Every branch should lead to an END node.
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
                {hasForm && (
                  <button
                    type="button"
                    onClick={onSave}
                    disabled={!valid || Boolean(customLabelError)}
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
