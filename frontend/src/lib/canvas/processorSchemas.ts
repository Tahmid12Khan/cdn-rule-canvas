// Zod schemas for the decision-node processor config forms (Task 13). These
// mirror the backend serde DTOs in BACKEND CONTRACT §6 (ProcessorConfig, serde
// tag="type", snake_case). The drawer forms use these via React Hook Form's
// zod resolver-style validation (here implemented with manual safeParse to keep
// dependencies minimal — RHF is not in package.json for the canvas subtree).
import { z } from "zod";

export const metaTagsOperator = z.enum(["contains", "equals", "exists"]);
export const deviceOperator = z.enum(["equals", "contains"]);
export const deviceValue = z.enum(["mobile", "desktop", "tablet"]);

// Meta Tags form: tag_name required; value required unless operator === "exists".
export const metaTagsSchema = z
  .object({
    type: z.literal("meta_tags"),
    tag_name: z
      .string()
      .trim()
      .min(1, "Enter the meta tag name to match (e.g. og:type)"),
    operator: metaTagsOperator,
    value: z.string().nullish(),
  })
  .superRefine((val, ctx) => {
    if (val.operator !== "exists") {
      if (!val.value || val.value.trim().length === 0) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          message:
            "Enter a value to compare against, or switch the operator to 'exists'",
          path: ["value"],
        });
      }
    }
  });

export const deviceTypeSchema = z.object({
  type: z.literal("device_type"),
  operator: deviceOperator,
  value: deviceValue,
});

export const articleUrlOperator = z.enum([
  "contains",
  "matches",
  "starts_with",
  "equals",
]);

// Article URL form: value required for every operator (regex pattern for "matches").
export const articleUrlSchema = z.object({
  type: z.literal("article_url"),
  operator: articleUrlOperator,
  value: z.string().trim().min(1, "Enter a value to compare against the URL"),
});

export type MetaTagsFormValues = z.infer<typeof metaTagsSchema>;
export type DeviceTypeFormValues = z.infer<typeof deviceTypeSchema>;
export type ArticleUrlFormValues = z.infer<typeof articleUrlSchema>;

export const processorSchema = z.union([
  metaTagsSchema,
  deviceTypeSchema,
  articleUrlSchema,
]);
