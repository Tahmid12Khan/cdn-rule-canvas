"use client";

// Searchable single-select product picker for the `product_select` field
// control (used by the `has_product` decision node). Debounces the query
// (300ms), searches Products by name case-insensitively (the backend `q`
// filter is ILIKE), and stores the selected product's LABEL. The current
// selection is shown as "name (label)".
//
// Browser state (open/query) + a network query → client component.
import { useEffect, useId, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { getProduct, searchProducts, type ProductRead } from "@/lib/api/products";
import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";

interface ProductSelectControlProps {
  id: string;
  value: string; // the stored label ("" when unset)
  onChange: (label: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

export function ProductSelectControl({
  id,
  value,
  onChange,
  disabled = false,
  placeholder = "Search products by name…",
}: ProductSelectControlProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const debounced = useDebouncedValue(query, 300);
  const listboxId = useId();
  const containerRef = useRef<HTMLDivElement>(null);

  const { data, isFetching } = useQuery({
    queryKey: ["products", "search", debounced],
    queryFn: () => searchProducts(debounced),
    enabled: open,
  });

  const options: ProductRead[] = data?.items ?? [];

  // Resolve the persisted label to its product so the current selection reads
  // "name (label)" rather than a bare label. Falls back to the label alone
  // while loading or when the label no longer resolves to a product.
  const { data: selectedProduct } = useQuery({
    queryKey: ["products", "get", value],
    queryFn: () => getProduct(value),
    enabled: value !== "",
  });
  const selectedLabel = selectedProduct
    ? `${selectedProduct.name} (${selectedProduct.label})`
    : value;

  // Close on outside click so the combobox behaves like a native dropdown.
  useEffect(() => {
    if (!open) return;
    function onClickOutside(e: MouseEvent) {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [open]);

  function select(label: string) {
    onChange(label);
    setOpen(false);
    setQuery("");
  }

  return (
    <div ref={containerRef} className="relative">
      {value && !open && (
        <div className="mb-1 flex items-center justify-between gap-2 rounded-md border border-status-prevBg px-3 py-2 text-sm">
          <span className="truncate text-nav">
            Selected:{" "}
            <code className="font-mono text-fg-muted">{selectedLabel}</code>
          </span>
          {!disabled && (
            <button
              type="button"
              onClick={() => onChange("")}
              className="shrink-0 text-xs font-medium text-danger hover:underline"
            >
              Clear
            </button>
          )}
        </div>
      )}

      <input
        id={id}
        type="text"
        role="combobox"
        aria-expanded={open}
        aria-controls={listboxId}
        autoComplete="off"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          if (!open) setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        disabled={disabled}
        placeholder={placeholder}
        className="w-full rounded-md border border-status-prevBg px-3 py-2 text-sm focus:border-brand-500 disabled:cursor-not-allowed disabled:opacity-60"
      />

      {open && (
        <ul
          id={listboxId}
          role="listbox"
          className="absolute z-10 mt-1 max-h-56 w-full overflow-auto rounded-md border border-status-prevBg bg-bg-elevated py-1 shadow-lg"
        >
          {isFetching && options.length === 0 && (
            <li className="px-3 py-2 text-sm text-status-prevFg">Searching…</li>
          )}
          {!isFetching && options.length === 0 && (
            <li className="px-3 py-2 text-sm text-status-prevFg">
              No matching products
            </li>
          )}
          {options.map((product) => (
            <li
              key={product.label}
              role="option"
              aria-selected={product.label === value}
            >
              <button
                type="button"
                onClick={() => select(product.label)}
                className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm text-nav hover:bg-status-prevBg/40"
              >
                <span className="truncate">{product.name}</span>
                <code className="shrink-0 font-mono text-xs text-fg-muted">
                  {product.label}
                </code>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
