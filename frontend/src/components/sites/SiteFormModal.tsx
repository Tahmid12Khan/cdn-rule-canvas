"use client";

// Client component: Radix Dialog hosting a controlled create/edit Site form
// (spec §6). Mirrors FeatureCreateModal's pattern — controlled React state
// validated with the same Zod schema (SiteCreate / SiteUpdate), since
// react-hook-form is not a declared dependency.
//
// One component serves both modes: when `site` is provided it edits (PATCH,
// slug locked); otherwise it creates (POST). The trigger is supplied by the
// caller so the same modal renders from the header button AND each card.
import { useEffect, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { ApiError } from "@/lib/api/client";
import {
  createSite,
  SiteCreate,
  SiteProtocol,
  type SiteRead,
  SiteUpdate,
  updateSite,
} from "@/lib/api/sites";
import { toUserError } from "@/lib/errors/userError";

type FieldKey =
  | "slug"
  | "name"
  | "source_protocol"
  | "source_host"
  | "source_port"
  | "dest_protocol"
  | "dest_host"
  | "dest_port";
type FieldErrors = Partial<Record<FieldKey | "form", string>>;

const PROTOCOLS = SiteProtocol.options;

const inputClass =
  "w-full rounded-md border border-status-prevBg px-3 py-2 text-sm text-nav shadow-sm focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500 disabled:cursor-not-allowed disabled:opacity-60";
const labelClass = "text-sm font-medium text-nav";
const errorClass = "text-xs font-medium text-danger";

interface SiteFormState {
  slug: string;
  name: string;
  source_protocol: SiteProtocol;
  source_host: string;
  source_port: string;
  dest_protocol: SiteProtocol;
  dest_host: string;
  dest_port: string;
}

function emptyState(): SiteFormState {
  return {
    slug: "",
    name: "",
    source_protocol: "http",
    source_host: "",
    source_port: "",
    dest_protocol: "http",
    dest_host: "",
    dest_port: "",
  };
}

function stateFromSite(site: SiteRead): SiteFormState {
  return {
    slug: site.slug,
    name: site.name,
    source_protocol: site.source_protocol,
    source_host: site.source_host,
    source_port: String(site.source_port),
    dest_protocol: site.dest_protocol,
    dest_host: site.dest_host,
    dest_port: String(site.dest_port),
  };
}

interface SiteFormModalProps {
  // Edit target; omit/undefined to create a new Site.
  site?: SiteRead;
  // Trigger element (create / empty-state usage). Omit when driving `open`
  // externally (the list opens the edit modal from a card action).
  trigger?: React.ReactNode;
  // Controlled open state. When provided the modal is fully driven by the
  // parent (no internal toggle); omit to self-manage via the trigger.
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function SiteFormModal({
  site,
  trigger,
  open: openProp,
  onOpenChange,
}: SiteFormModalProps) {
  const isEdit = site !== undefined;
  const [internalOpen, setInternalOpen] = useState(false);
  const isControlled = openProp !== undefined;
  const open = isControlled ? openProp : internalOpen;
  const setOpen = (next: boolean) => {
    if (!isControlled) setInternalOpen(next);
    onOpenChange?.(next);
  };
  const [form, setForm] = useState<SiteFormState>(() =>
    site ? stateFromSite(site) : emptyState(),
  );
  const [errors, setErrors] = useState<FieldErrors>({});

  const queryClient = useQueryClient();

  // Re-seed the form whenever the dialog opens so an edit modal always reflects
  // the latest site and a create modal starts blank.
  useEffect(() => {
    if (open) {
      setForm(site ? stateFromSite(site) : emptyState());
      setErrors({});
    }
  }, [open, site]);

  const mutation = useMutation({
    mutationFn: (body: SiteCreate | SiteUpdate) =>
      isEdit
        ? updateSite(site.slug, body as SiteUpdate)
        : createSite(body as SiteCreate),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["sites"] });
      setOpen(false);
    },
    onError: (err: unknown) => {
      // The backend returns 409 SLUG_CONFLICT for ANY uniqueness violation on a
      // Site (slug PK, name unique, or (source_host, source_port) unique) — not
      // just the slug. The generic version helper's 409/SLUG_CONFLICT copy is
      // version-centric ("Save as New Version…" / "That slug is already taken"),
      // so handle a site conflict here with sites-aware copy instead. The slug
      // field gets a hint since it's the most common collision.
      if (err instanceof ApiError && err.status === 409) {
        setErrors({
          form:
            "A site with this slug, name, or source host:port already exists. " +
            "Change the conflicting value and try again.",
          slug: "This slug may already be in use",
        });
        return;
      }
      const ue = toUserError(err, { surface: isEdit ? "save" : "create" });
      setErrors({ form: `${ue.title}. ${ue.howToFix}` });
    },
  });

  function setField<K extends keyof SiteFormState>(
    key: K,
    value: SiteFormState[K],
  ) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const candidate = {
      slug: form.slug,
      name: form.name,
      source_protocol: form.source_protocol,
      source_host: form.source_host,
      source_port: form.source_port,
      dest_protocol: form.dest_protocol,
      dest_host: form.dest_host,
      dest_port: form.dest_port,
    };
    const parsed = SiteCreate.safeParse(candidate);
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
    if (isEdit) {
      // slug is immutable; PATCH the rest.
      const { slug: _slug, ...rest } = parsed.data;
      void _slug;
      mutation.mutate(rest satisfies SiteUpdate);
    } else {
      mutation.mutate(parsed.data);
    }
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      {trigger && <Dialog.Trigger asChild>{trigger}</Dialog.Trigger>}

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-full max-w-md -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-lg bg-bg-elevated p-6 shadow-xl focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-nav">
            {isEdit ? "Edit Site" : "Add A New Site"}
          </Dialog.Title>
          <Dialog.Description className="mt-1 text-sm text-status-prevFg">
            Route an incoming source host to a destination upstream.
          </Dialog.Description>

          <form className="mt-5 flex flex-col gap-4" onSubmit={handleSubmit}>
            <div className="flex flex-col gap-1">
              <label htmlFor="site-slug" className={labelClass}>
                Slug
              </label>
              <input
                id="site-slug"
                name="slug"
                value={form.slug}
                onChange={(e) => setField("slug", e.target.value)}
                placeholder="demo-localhost"
                disabled={isEdit}
                aria-invalid={errors.slug ? true : undefined}
                className={inputClass}
              />
              {errors.slug && <p className={errorClass}>{errors.slug}</p>}
            </div>

            <div className="flex flex-col gap-1">
              <label htmlFor="site-name" className={labelClass}>
                Name
              </label>
              <input
                id="site-name"
                name="name"
                value={form.name}
                onChange={(e) => setField("name", e.target.value)}
                placeholder="Demo (localhost:9000)"
                aria-invalid={errors.name ? true : undefined}
                className={inputClass}
              />
              {errors.name && <p className={errorClass}>{errors.name}</p>}
            </div>

            <fieldset className="flex flex-col gap-2 rounded-md border border-status-prevBg p-3">
              <legend className="px-1 text-xs font-semibold uppercase tracking-wide text-status-prevFg">
                Source
              </legend>
              <div className="grid grid-cols-[auto_1fr_auto] gap-2">
                <div className="flex flex-col gap-1">
                  <label htmlFor="site-source-protocol" className={labelClass}>
                    Protocol
                  </label>
                  <select
                    id="site-source-protocol"
                    name="source_protocol"
                    value={form.source_protocol}
                    onChange={(e) =>
                      setField("source_protocol", e.target.value as SiteProtocol)
                    }
                    className={inputClass}
                  >
                    {PROTOCOLS.map((p) => (
                      <option key={p} value={p}>
                        {p}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="flex flex-col gap-1">
                  <label htmlFor="site-source-host" className={labelClass}>
                    Host
                  </label>
                  <input
                    id="site-source-host"
                    name="source_host"
                    value={form.source_host}
                    onChange={(e) => setField("source_host", e.target.value)}
                    placeholder="localhost"
                    aria-invalid={errors.source_host ? true : undefined}
                    className={inputClass}
                  />
                </div>
                <div className="flex flex-col gap-1">
                  <label htmlFor="site-source-port" className={labelClass}>
                    Port
                  </label>
                  <input
                    id="site-source-port"
                    name="source_port"
                    inputMode="numeric"
                    value={form.source_port}
                    onChange={(e) => setField("source_port", e.target.value)}
                    placeholder="9000"
                    aria-invalid={errors.source_port ? true : undefined}
                    className={`${inputClass} w-24`}
                  />
                </div>
              </div>
              {errors.source_host && (
                <p className={errorClass}>{errors.source_host}</p>
              )}
              {errors.source_port && (
                <p className={errorClass}>{errors.source_port}</p>
              )}
            </fieldset>

            <fieldset className="flex flex-col gap-2 rounded-md border border-status-prevBg p-3">
              <legend className="px-1 text-xs font-semibold uppercase tracking-wide text-status-prevFg">
                Destination
              </legend>
              <div className="grid grid-cols-[auto_1fr_auto] gap-2">
                <div className="flex flex-col gap-1">
                  <label htmlFor="site-dest-protocol" className={labelClass}>
                    Protocol
                  </label>
                  <select
                    id="site-dest-protocol"
                    name="dest_protocol"
                    value={form.dest_protocol}
                    onChange={(e) =>
                      setField("dest_protocol", e.target.value as SiteProtocol)
                    }
                    className={inputClass}
                  >
                    {PROTOCOLS.map((p) => (
                      <option key={p} value={p}>
                        {p}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="flex flex-col gap-1">
                  <label htmlFor="site-dest-host" className={labelClass}>
                    Host
                  </label>
                  <input
                    id="site-dest-host"
                    name="dest_host"
                    value={form.dest_host}
                    onChange={(e) => setField("dest_host", e.target.value)}
                    placeholder="demo-upstream"
                    aria-invalid={errors.dest_host ? true : undefined}
                    className={inputClass}
                  />
                </div>
                <div className="flex flex-col gap-1">
                  <label htmlFor="site-dest-port" className={labelClass}>
                    Port
                  </label>
                  <input
                    id="site-dest-port"
                    name="dest_port"
                    inputMode="numeric"
                    value={form.dest_port}
                    onChange={(e) => setField("dest_port", e.target.value)}
                    placeholder="8081"
                    aria-invalid={errors.dest_port ? true : undefined}
                    className={`${inputClass} w-24`}
                  />
                </div>
              </div>
              {errors.dest_host && (
                <p className={errorClass}>{errors.dest_host}</p>
              )}
              {errors.dest_port && (
                <p className={errorClass}>{errors.dest_port}</p>
              )}
            </fieldset>

            {errors.form && (
              <p role="alert" className={errorClass}>
                {errors.form}
              </p>
            )}

            <div className="mt-2 flex justify-end gap-3">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="rounded-md border border-status-prevBg px-4 py-2 text-sm font-medium text-nav transition hover:bg-status-prevBg/30"
                >
                  Cancel
                </button>
              </Dialog.Close>
              <button
                type="submit"
                disabled={mutation.isPending}
                className="inline-flex items-center rounded-md bg-action-600 px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-action-700 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {mutation.isPending
                  ? isEdit
                    ? "Saving…"
                    : "Creating…"
                  : isEdit
                    ? "Save Site"
                    : "Create Site"}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
