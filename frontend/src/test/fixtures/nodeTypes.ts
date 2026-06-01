// Node-type manifest fixture for tests (mirrors backend/config/node_types.json,
// verbatim shape). Tests must NOT depend on a live backend; MSW serves this for
// GET /api/v1/node-types and components/forms validate against it. ALL keys are
// snake_case.
import type { NodeManifest } from "@/lib/api/nodeTypes";
import type { ProcessorConfig } from "@/lib/canvas/types";

export const NODE_TYPES_FIXTURE: NodeManifest = {
  categories: [
    { id: "session", label: "Session" },
    { id: "user", label: "User", coming_soon: true },
    { id: "content", label: "Content" },
    { id: "advanced", label: "Advanced", coming_soon: true },
  ],
  node_types: [
    {
      kind: "meta_tags",
      label: "Meta Tags",
      category: "content",
      summary: "Matches against a meta tag in the request HTML by name.",
      fields: [
        {
          name: "tag_name",
          label: "Tag name",
          control: "text",
          required: true,
          default: "",
          placeholder: "e.g. paywall",
          required_message: "Enter the meta tag name to match (e.g. og:type)",
        },
        {
          name: "operator",
          label: "Operator",
          control: "select",
          required: true,
          default: "contains",
          options: [
            { value: "contains", label: "contains" },
            { value: "equals", label: "equals" },
            { value: "exists", label: "exists" },
          ],
        },
        {
          name: "value",
          label: "Value",
          control: "text",
          required: false,
          required_unless: { field: "operator", value: "exists" },
          default: "",
          placeholder: "e.g. true",
          required_message:
            "Enter a value to compare against, or switch the operator to 'exists'",
        },
      ],
      output: {
        branches: [
          { id: "yes", label: "Yes" },
          { id: "no", label: "No" },
        ],
      },
    },
    {
      kind: "device_type",
      label: "Device Type",
      category: "session",
      summary:
        "Matches against the request device type (mobile, desktop, tablet).",
      fields: [
        {
          name: "operator",
          label: "Operator",
          control: "select",
          required: true,
          default: "equals",
          options: [
            { value: "equals", label: "equals" },
            { value: "contains", label: "contains" },
          ],
        },
        {
          name: "value",
          label: "Value",
          control: "select",
          required: true,
          default: "mobile",
          options: [
            { value: "mobile", label: "mobile" },
            { value: "desktop", label: "desktop" },
            { value: "tablet", label: "tablet" },
          ],
        },
      ],
      output: {
        branches: [
          { id: "yes", label: "Yes" },
          { id: "no", label: "No" },
        ],
      },
    },
    {
      kind: "article_url",
      label: "Article URL",
      category: "content",
      summary: "Matches against the request article URL (path).",
      fields: [
        {
          name: "operator",
          label: "Operator",
          control: "select",
          required: true,
          default: "contains",
          options: [
            { value: "contains", label: "contains" },
            { value: "matches", label: "matches" },
            { value: "starts_with", label: "starts_with" },
            { value: "equals", label: "equals" },
          ],
        },
        {
          name: "value",
          label: "Value",
          control: "text",
          required: true,
          default: "",
          placeholder: "e.g. /article",
          required_message: "Enter a value to compare against the URL",
        },
      ],
      output: {
        branches: [
          { id: "yes", label: "Yes" },
          { id: "no", label: "No" },
        ],
      },
    },
  ],
};

// Convenience default processor configs for tests (the dropped-node defaults the
// palette would produce). meta_tags is intentionally incomplete (tag_name "").
export const META_TAGS_DEFAULT: ProcessorConfig = {
  type: "meta_tags",
  tag_name: "",
  operator: "contains",
  value: "",
};
