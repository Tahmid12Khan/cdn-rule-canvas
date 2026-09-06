"use client";

// Outcomes Library list (Outcomes Library design). Owns the list interactivity
// — TanStack Query for the page + search, the create dialog (slug + name +
// component pick; the version/variables are configured afterwards in the
// editor), and delete confirmation. Mirrors ComponentLibraryClient's grid
// layout + ProductCatalogueClient's search-input pattern.
import { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import { CardGridSkeleton } from "@/components/ui/CardGridSkeleton";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorBanner } from "@/components/ui/ErrorBanner";
import { Pagination } from "@/components/ui/Pagination";
import { ApiError } from "@/lib/api/client";
import { listComponentTemplates } from "@/lib/api/componentTemplates";
import {
  createSavedOutcome,
  deleteSavedOutcome,
  listSavedOutcomes,
  savedOutcomeKeys,
  SavedOutcomeCreate,
  type SavedOutcomeRead,
} from "@/lib/api/savedOutcomes";
import { toUserError, type UserError } from "@/lib/errors/userError";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";

const PAGE_SIZE = 20;

type SavedOutcomesPage = Awaited<ReturnType<typeof listSavedOutcomes>>;

interface SavedOutcomeLibraryClientProps {
  initialOutcomes?: SavedOutcomesPage;
}

const inputClass =
  "w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg shadow-sm focus:border-accent focus:outline-none focus:ring-1 focus:ring-accent";
const labelClass = "text-sm font-medium text-fg";
const errorClass = "text-xs font-medium text-danger";

function versionLabel(outcome: SavedOutcomeRead): string {
  return outcome.version_number === null
    ? "Latest"
    : `v${outcome.version_number}`;
}

export function SavedOutcomeLibraryClient({
  initialOutcomes,
}: SavedOutcomeLibraryClientProps = {}) {
  const router = useRouter();
  const queryClient = useQueryClient();

  const [page, setPage] = useState(1);
  const [query, setQuery] = useState("");
  const debouncedQuery = useDebouncedValue(query, 300);
  const [createOpen, setCreateOpen] = useState(false);
  const [deleting, setDeleting] = useState<SavedOutcomeRead | null>(null);
  const [deleteError, setDeleteError] = useState<UserError | null>(null);

  const listParams = { page, page_size: PAGE_SIZE, q: debouncedQuery };
  const { data, isPending, isError, error, refetch, isFetching } = useQuery({
    queryKey: savedOutcomeKeys.list(listParams),
    queryFn: () => listSavedOutcomes(listParams),
    placeholderData: keepPreviousData,
    initialData:
      page === 1 && debouncedQuery === "" ? initialOutcomes : undefined,
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteSavedOutcome(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["saved-outcomes"] });
      setDeleting(null);
      setDeleteError(null);
    },
    onError: (err: unknown) => setDeleteError(toUserError(err, { surface: "delete" })),
  });

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-fg">Outcomes</h1>
          <p className="mt-1 text-sm text-fg-muted">
            Named, versioned references to a Library Component with filled-in
            variable values. Rule nodes pick a saved outcome to inject.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setCreateOpen(true)}
          className="inline-flex items-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-fg shadow-sm transition hover:opacity-90"
        >
          + New outcome
        </button>
      </div>

      <input
        type="search"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setPage(1);
        }}
        placeholder="Search outcomes by name…"
        aria-label="Search outcomes"
        className={`${inputClass} max-w-sm`}
      />

      {isError && (
        <ErrorBanner
          error={toUserError(error, { surface: "load" })}
          onRetry={() => void refetch()}
        />
      )}

      {isPending && !isError && <CardGridSkeleton />}

      {!isPending && !isError && data.items.length === 0 && (
        <EmptyState
          title="No outcomes yet"
          description="Save a component + version + variable values as a reusable named outcome."
          action={
            <button
              type="button"
              onClick={() => setCreateOpen(true)}
              className="inline-flex items-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-fg shadow-sm transition hover:opacity-90"
            >
              + New outcome
            </button>
          }
        />
      )}

      {!isPending && !isError && data.items.length > 0 && (
        <>
          <div
            className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
            aria-busy={isFetching}
          >
            {data.items.map((outcome) => (
              <div
                key={outcome.id}
                className="flex flex-col gap-3 rounded-lg border border-border bg-bg-elevated p-5"
              >
                <div className="flex items-start justify-between gap-3">
                  <Link
                    href={`/products/outcomes/${outcome.slug}`}
                    className="text-base font-semibold text-fg hover:text-accent-onMuted"
                  >
                    {outcome.name}
                  </Link>
                  <code className="shrink-0 rounded-full bg-bg-overlay px-2 py-0.5 font-mono text-xs text-accent-onMuted">
                    {outcome.slug}
                  </code>
                </div>

                <div className="flex items-center gap-2 text-xs text-fg-muted">
                  <span className="rounded-full bg-bg-overlay px-2 py-0.5 font-medium">
                    {outcome.component_name}
                  </span>
                  <span>{versionLabel(outcome)}</span>
                </div>

                <div className="mt-1 flex items-center gap-2">
                  <Link
                    href={`/products/outcomes/${outcome.slug}`}
                    className="rounded-md border border-border px-3 py-1 text-xs font-medium text-fg transition hover:bg-bg-overlay"
                  >
                    Edit
                  </Link>
                  <button
                    type="button"
                    onClick={() => {
                      setDeleteError(null);
                      setDeleting(outcome);
                    }}
                    className="rounded-md border border-border px-3 py-1 text-xs font-medium text-danger transition hover:bg-danger/10"
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
          <Pagination
            page={data.page}
            pageSize={data.page_size}
            total={data.total}
            onPageChange={setPage}
          />
        </>
      )}

      <CreateOutcomeModal
        open={createOpen}
        onOpenChange={setCreateOpen}
        onCreated={(slug) => router.push(`/products/outcomes/${slug}`)}
      />

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
            <Dialog.Title className="text-lg font-semibold text-fg">
              Delete outcome {deleting?.name}?
            </Dialog.Title>
            <Dialog.Description className="mt-1 text-sm text-fg-muted">
              This removes{" "}
              <code className="font-mono text-fg-muted">{deleting?.slug}</code>{" "}
              from the library. Rule nodes referencing it will fail open. This
              cannot be undone.
            </Dialog.Description>

            {deleteError && (
              <div className="mt-4">
                <ErrorBanner error={deleteError} />
              </div>
            )}

            <div className="mt-6 flex justify-end gap-2">
              <Dialog.Close asChild>
                <button
                  type="button"
                  disabled={deleteMutation.isPending}
                  className="rounded-lg border border-border px-3.5 py-2 text-sm font-medium text-fg hover:bg-bg-overlay disabled:opacity-50"
                >
                  Cancel
                </button>
              </Dialog.Close>
              <button
                type="button"
                onClick={() => {
                  if (deleting) deleteMutation.mutate(deleting.id);
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

type FieldKey = "slug" | "name" | "component_id";
type FieldErrors = Partial<Record<FieldKey | "form", string>>;

interface CreateOutcomeModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: (slug: string) => void;
}

function CreateOutcomeModal({
  open,
  onOpenChange,
  onCreated,
}: CreateOutcomeModalProps) {
  const queryClient = useQueryClient();
  const [slug, setSlug] = useState("");
  const [name, setName] = useState("");
  const [componentId, setComponentId] = useState("");
  const [errors, setErrors] = useState<FieldErrors>({});

  const componentsQuery = useQuery({
    queryKey: ["component-templates", "list", { page: 1, page_size: 100 }],
    queryFn: () => listComponentTemplates({ page: 1, page_size: 100 }),
    enabled: open,
  });
  const components = componentsQuery.data?.items ?? [];

  const mutation = useMutation({
    mutationFn: (body: SavedOutcomeCreate) => createSavedOutcome(body),
  });

  function reset() {
    setSlug("");
    setName("");
    setComponentId("");
    setErrors({});
  }

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const candidate = { slug, name, component_id: componentId, variables: {} };
    const parsed = SavedOutcomeCreate.safeParse(candidate);
    if (!parsed.success) {
      const next: FieldErrors = {};
      for (const issue of parsed.error.issues) {
        const field = issue.path[0];
        if (typeof field === "string") {
          next[field as FieldKey] = next[field as FieldKey] ?? issue.message;
        }
      }
      setErrors(next);
      return;
    }
    setErrors({});
    mutation.mutate(parsed.data, {
      onSuccess: (created) => {
        void queryClient.invalidateQueries({ queryKey: ["saved-outcomes"] });
        reset();
        onOpenChange(false);
        onCreated(created.slug);
      },
      onError: (err: unknown) => {
        if (err instanceof ApiError && err.status === 409) {
          setErrors({
            form: "An outcome with this slug already exists.",
            slug: "This slug may already be in use",
          });
          return;
        }
        const ue = toUserError(err, { surface: "create" });
        setErrors({ form: `${ue.title}. ${ue.howToFix}` });
      },
    });
  }

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) reset();
        onOpenChange(next);
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-full max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-fg">
            New outcome
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-fg-muted">
            Pick a component to reference. You&apos;ll choose the version and
            fill in variables next.
          </Dialog.Description>

          <form className="mt-5 flex flex-col gap-4" onSubmit={handleSubmit}>
            <div className="flex flex-col gap-1">
              <label htmlFor="outcome-slug" className={labelClass}>
                Slug
              </label>
              <input
                id="outcome-slug"
                name="slug"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
                placeholder="promo-banner-default"
                aria-invalid={errors.slug ? true : undefined}
                className={inputClass}
              />
              {errors.slug && <p className={errorClass}>{errors.slug}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="outcome-name" className={labelClass}>
                Name
              </label>
              <input
                id="outcome-name"
                name="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Promo Banner (Default)"
                aria-invalid={errors.name ? true : undefined}
                className={inputClass}
              />
              {errors.name && <p className={errorClass}>{errors.name}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="outcome-component" className={labelClass}>
                Component
              </label>
              <select
                id="outcome-component"
                name="component_id"
                value={componentId}
                onChange={(e) => setComponentId(e.target.value)}
                aria-invalid={errors.component_id ? true : undefined}
                className={inputClass}
              >
                <option value="">
                  {components.length === 0
                    ? "No components yet"
                    : "Select a component…"}
                </option>
                {components.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
              {errors.component_id && (
                <p className={errorClass}>{errors.component_id}</p>
              )}
            </div>

            {errors.form && (
              <p role="alert" className={errorClass}>
                {errors.form}
              </p>
            )}

            <div className="mt-2 flex justify-end gap-3">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="rounded-md border border-border px-4 py-2 text-sm font-medium text-fg transition hover:bg-bg-overlay"
                >
                  Cancel
                </button>
              </Dialog.Close>
              <button
                type="submit"
                disabled={mutation.isPending}
                className="inline-flex items-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-fg shadow-sm transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {mutation.isPending ? "Creating…" : "Create outcome"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
