"use client";

// Client component: owns the Product Catalogue list interactivity — TanStack
// Query for the list + search, inline edit (name/description only — label is
// immutable), a delete confirmation, and the "New product" create modal.
// Mirrors SitesListClient's list/query/modal composition.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { ProductCreateModal } from "@/components/product-catalogue/ProductCreateModal";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { Pagination } from "@/components/ui/Pagination";
import {
  deleteProduct,
  listProducts,
  type ProductRead,
  ProductUpdate,
  updateProduct,
} from "@/lib/api/products";
import { toUserError } from "@/lib/errors/userError";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";

const PAGE_SIZE = 20;

type ProductsPage = Awaited<ReturnType<typeof listProducts>>;

interface ProductCatalogueClientProps {
  // SSR-prefetched first page. Seeds initialData on the page-1, empty-search key
  // so the list hydrates without a client-side fetch waterfall.
  initialProducts?: ProductsPage;
}

const inputClass =
  "w-full rounded-md border border-status-prevBg px-2 py-1 text-sm text-nav shadow-sm focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500";

export function ProductCatalogueClient({
  initialProducts,
}: ProductCatalogueClientProps = {}) {
  const [page, setPage] = useState(1);
  const [query, setQuery] = useState("");
  const debouncedQuery = useDebouncedValue(query, 300);
  const [editingLabel, setEditingLabel] = useState<string | null>(null);
  const [editForm, setEditForm] = useState({ name: "", description: "" });
  const [editError, setEditError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<ProductRead | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const queryClient = useQueryClient();

  const { data, isPending, isError, error, refetch, isFetching } = useQuery({
    queryKey: ["products", { page, page_size: PAGE_SIZE, q: debouncedQuery }],
    queryFn: () => listProducts({ page, page_size: PAGE_SIZE, q: debouncedQuery }),
    placeholderData: keepPreviousData,
    initialData: page === 1 && debouncedQuery === "" ? initialProducts : undefined,
  });

  const updateMutation = useMutation({
    mutationFn: ({ label, body }: { label: string; body: ProductUpdate }) =>
      updateProduct(label, body),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["products"] });
      setEditingLabel(null);
      setEditError(null);
    },
    onError: (err: unknown) => {
      const ue = toUserError(err, { surface: "save" });
      setEditError(`${ue.title}. ${ue.howToFix}`);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (label: string) => deleteProduct(label),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["products"] });
      setDeleting(null);
      setDeleteError(null);
    },
    onError: (err: unknown) => {
      const ue = toUserError(err, { surface: "delete" });
      setDeleteError(`${ue.title}. ${ue.howToFix}`);
    },
  });

  function startEdit(product: ProductRead) {
    setEditingLabel(product.label);
    setEditForm({ name: product.name, description: product.description ?? "" });
    setEditError(null);
  }

  function cancelEdit() {
    setEditingLabel(null);
    setEditError(null);
  }

  function saveEdit(label: string) {
    const parsed = ProductUpdate.safeParse({
      name: editForm.name,
      description: editForm.description.trim() === "" ? undefined : editForm.description,
    });
    if (!parsed.success) {
      setEditError(parsed.error.issues[0]?.message ?? "Invalid input");
      return;
    }
    updateMutation.mutate({ label, body: parsed.data });
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-nav">Product Catalogue</h1>
          <p className="mt-1 text-sm text-status-prevFg">
            Products matched by the `has_product` decision node on the Rule
            Canvas.
          </p>
        </div>
        <ProductCreateModal />
      </div>

      <input
        type="search"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setPage(1);
        }}
        placeholder="Search products by name…"
        aria-label="Search products"
        className={`${inputClass} max-w-sm`}
      />

      {isError && (
        <ErrorBanner
          error={toUserError(error, { surface: "load" })}
          onRetry={() => void refetch()}
        />
      )}

      {isPending && !isError && (
        <p className="text-sm text-status-prevFg">Loading products…</p>
      )}

      {!isPending && !isError && data.items.length === 0 && (
        <p className="text-sm text-status-prevFg">
          {debouncedQuery
            ? "No products match your search."
            : "No products yet. Add your first product to match it with a has_product node."}
        </p>
      )}

      {!isPending && !isError && data.items.length > 0 && (
        <>
          <table className="w-full text-left text-sm" aria-busy={isFetching}>
            <thead>
              <tr className="border-b border-status-prevBg text-xs font-semibold uppercase tracking-wide text-status-prevFg">
                <th className="py-2 pr-4">Label</th>
                <th className="py-2 pr-4">Name</th>
                <th className="py-2 pr-4">Description</th>
                <th className="py-2 pr-4">Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.items.map((product) => {
                const isEditing = editingLabel === product.label;
                return (
                  <tr key={product.label} className="border-b border-status-prevBg/50">
                    <td className="py-2 pr-4">
                      <code className="font-mono text-xs text-fg-muted">
                        {product.label}
                      </code>
                    </td>
                    <td className="py-2 pr-4">
                      {isEditing ? (
                        <input
                          aria-label="Name"
                          value={editForm.name}
                          onChange={(e) =>
                            setEditForm((prev) => ({ ...prev, name: e.target.value }))
                          }
                          className={inputClass}
                        />
                      ) : (
                        <span className="text-nav">{product.name}</span>
                      )}
                    </td>
                    <td className="py-2 pr-4">
                      {isEditing ? (
                        <input
                          aria-label="Description"
                          value={editForm.description}
                          onChange={(e) =>
                            setEditForm((prev) => ({
                              ...prev,
                              description: e.target.value,
                            }))
                          }
                          className={inputClass}
                        />
                      ) : (
                        <span className="text-status-prevFg">
                          {product.description ?? "—"}
                        </span>
                      )}
                    </td>
                    <td className="py-2 pr-4">
                      {isEditing ? (
                        <div className="flex items-center gap-2">
                          <button
                            type="button"
                            onClick={() => saveEdit(product.label)}
                            disabled={updateMutation.isPending}
                            className="rounded-md bg-action-600 px-3 py-1 text-xs font-medium text-white hover:bg-action-700 disabled:opacity-60"
                          >
                            {updateMutation.isPending ? "Saving…" : "Save"}
                          </button>
                          <button
                            type="button"
                            onClick={cancelEdit}
                            className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-nav hover:bg-status-prevBg/30"
                          >
                            Cancel
                          </button>
                        </div>
                      ) : (
                        <div className="flex items-center gap-2">
                          <button
                            type="button"
                            onClick={() => startEdit(product)}
                            className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-nav transition hover:bg-status-prevBg/30"
                          >
                            Edit
                          </button>
                          <button
                            type="button"
                            onClick={() => {
                              setDeleteError(null);
                              setDeleting(product);
                            }}
                            className="rounded-md border border-status-prevBg px-3 py-1 text-xs font-medium text-danger transition hover:bg-danger/10"
                          >
                            Delete
                          </button>
                        </div>
                      )}
                      {isEditing && editError && (
                        <p role="alert" className="mt-1 text-xs font-medium text-danger">
                          {editError}
                        </p>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <Pagination
            page={data.page}
            pageSize={data.page_size}
            total={data.total}
            onPageChange={setPage}
          />
        </>
      )}

      <Dialog.Root
        open={deleting !== null}
        onOpenChange={(next) => {
          if (!deleteMutation.isPending && !next) {
            setDeleting(null);
            setDeleteError(null);
          }
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[90vw] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl bg-bg-elevated p-6 shadow-xl focus:outline-none">
            <Dialog.Title className="text-lg font-semibold text-nav">
              Delete Product {deleting?.name}?
            </Dialog.Title>
            <Dialog.Description className="mt-1 text-sm text-status-prevFg">
              This removes{" "}
              <code className="font-mono text-fg-muted">{deleting?.label}</code>{" "}
              from the catalogue. Any `has_product` node matching it will stop
              matching. This action cannot be undone.
            </Dialog.Description>

            {deleteError && (
              <p role="alert" className="mt-4 text-xs font-medium text-danger">
                {deleteError}
              </p>
            )}

            <div className="mt-6 flex justify-end gap-2">
              <Dialog.Close asChild>
                <button
                  type="button"
                  disabled={deleteMutation.isPending}
                  className="rounded-lg border border-status-prevBg px-3.5 py-2 text-sm font-medium text-nav hover:bg-status-prevBg disabled:opacity-50"
                >
                  Cancel
                </button>
              </Dialog.Close>
              <button
                type="button"
                onClick={() => {
                  if (deleting) deleteMutation.mutate(deleting.label);
                }}
                disabled={deleteMutation.isPending}
                className="rounded-lg bg-danger px-3.5 py-2 text-sm font-semibold text-white hover:opacity-90 disabled:opacity-50"
              >
                {deleteMutation.isPending ? "Deleting…" : "Delete"}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  );
}
