"use client";

// Backend-driven node metadata (spec Part E). Fetches the node-type manifest
// (GET /api/v1/node-types) via TanStack Query and exposes a kind -> spec index
// for O(1) lookup in nodes, palette, and config forms. Cached for the session;
// the manifest is small + cacheable so a long staleTime is appropriate.
import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";

import {
  getNodeTypes,
  type NodeManifest,
  type NodeTypeSpec,
} from "@/lib/api/nodeTypes";

export interface NodeTypesResult {
  manifest: NodeManifest | undefined;
  // kind -> spec, undefined until the manifest loads.
  specByKind: (kind: string) => NodeTypeSpec | undefined;
  isLoading: boolean;
  isError: boolean;
}

const NODE_TYPES_QUERY_KEY = ["node-types"] as const;

export function useNodeTypes(): NodeTypesResult {
  const { data, isLoading, isError } = useQuery({
    queryKey: NODE_TYPES_QUERY_KEY,
    queryFn: getNodeTypes,
    // Manifest changes only on a backend deploy; keep it fresh for the session.
    staleTime: Infinity,
    gcTime: Infinity,
  });

  const index = useMemo(() => {
    const map = new Map<string, NodeTypeSpec>();
    for (const spec of data?.node_types ?? []) map.set(spec.kind, spec);
    return map;
  }, [data]);

  const specByKind = useMemo(
    () => (kind: string) => index.get(kind),
    [index],
  );

  return { manifest: data, specByKind, isLoading, isError };
}
