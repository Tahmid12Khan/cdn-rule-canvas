"use client";

// Client component: controlled search box, debounces the term (300ms) before
// reporting it upward so the parent can drive the server-side ?search= query.
import { useEffect, useState } from "react";

import { useDebouncedValue } from "@/lib/hooks/useDebouncedValue";

interface SearchInputProps {
  onSearch: (term: string) => void;
  placeholder?: string;
}

export function SearchInput({
  onSearch,
  placeholder = "Search versions",
}: SearchInputProps) {
  const [term, setTerm] = useState("");
  const debounced = useDebouncedValue(term, 300);

  useEffect(() => {
    onSearch(debounced.trim());
  }, [debounced, onSearch]);

  return (
    <div className="relative">
      <svg
        aria-hidden
        viewBox="0 0 20 20"
        fill="none"
        className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-status-prev"
      >
        <circle cx="9" cy="9" r="6" stroke="currentColor" strokeWidth="2" />
        <path
          d="m14 14 4 4"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
        />
      </svg>
      <input
        type="search"
        value={term}
        onChange={(e) => setTerm(e.target.value)}
        placeholder={placeholder}
        aria-label="Search versions"
        className="w-full rounded-lg border border-status-prevBg bg-bg-elevated py-2 pl-9 pr-3 text-sm text-nav placeholder:text-status-prev focus:border-brand-500 focus:outline-none focus:ring-1 focus:ring-brand-500 sm:w-64"
      />
    </div>
  );
}
