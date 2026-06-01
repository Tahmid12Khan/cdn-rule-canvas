import { z } from "zod";

import { apiGet } from "@/lib/api/client";

export const HealthSchema = z.object({
  status: z.string(),
  version: z.string().optional(),
  git_commit: z.string().optional(),
});
export type Health = z.infer<typeof HealthSchema>;

export const fetchHealth = (): Promise<Health> =>
  apiGet("/health", HealthSchema);
