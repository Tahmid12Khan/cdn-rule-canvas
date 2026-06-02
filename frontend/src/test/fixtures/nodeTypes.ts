// Node-type manifest fixture for tests (mirrors backend/config/node_types.json,
// verbatim shape). Tests must NOT depend on a live backend; MSW serves this for
// GET /api/v1/node-types and components/forms validate against it. ALL keys are
// snake_case.
import type { NodeManifest } from "@/lib/api/nodeTypes";
import type { ProcessorConfig } from "@/lib/canvas/types";

export const NODE_TYPES_FIXTURE: NodeManifest = {
  display: { value_max_chars: 10 },
  categories: [
    { id: "session", label: "Session" },
    { id: "user", label: "User", coming_soon: true },
    { id: "content", label: "Content" },
    { id: "json", label: "JSON" },
    { id: "advanced", label: "Advanced", coming_soon: true },
  ],
  // node_kind defaults to "decision" when omitted; the three expression action
  // types are declared at the end (expression-nodes-spec §2).
  node_types: [
    {
      kind: "meta_tags",
      label: "Meta Tags",
      category: "content",
      applies_to: "html",
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
            { value: "contains", label: "contains", symbol: "⊃" },
            { value: "equals", label: "equals", symbol: "==" },
            { value: "exists", label: "exists", symbol: "∃" },
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
      applies_to: "all",
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
            { value: "equals", label: "equals", symbol: "==" },
            { value: "contains", label: "contains", symbol: "⊃" },
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
      label: "URL",
      category: "content",
      applies_to: "all",
      summary: "Matches against the request URL (path).",
      fields: [
        {
          name: "operator",
          label: "Operator",
          control: "select",
          required: true,
          default: "contains",
          options: [
            { value: "contains", label: "contains", symbol: "⊃" },
            { value: "matches", label: "matches", symbol: "~=" },
            { value: "starts_with", label: "starts_with", symbol: "^=" },
            { value: "equals", label: "equals", symbol: "==" },
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
    {
      kind: "json_expression",
      label: "JSON Expression",
      category: "json",
      applies_to: "json",
      summary: "Matches a JSONPath in the response body against a value.",
      fields: [
        {
          name: "json_path",
          label: "JSON path",
          control: "text",
          required: true,
          default: "",
          placeholder: "$.type",
          required_message: "Enter a JSONPath (e.g. $.type)",
        },
        {
          name: "operator",
          label: "Operator",
          control: "select",
          required: true,
          default: "contains",
          options: [
            { value: "equals", label: "equals", symbol: "==" },
            { value: "contains", label: "contains", symbol: "⊃" },
            { value: "exists", label: "exists", symbol: "∃" },
          ],
        },
        {
          name: "value",
          label: "Value",
          control: "text",
          required: false,
          required_unless: { field: "operator", value: "exists" },
          default: "",
          placeholder: "e.g. premium",
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
      kind: "trim_json",
      label: "Trim JSON",
      category: "json",
      applies_to: "json",
      node_kind: "expression",
      summary:
        "Trims a JSON array at a path to at most N items (min(actual, length)).",
      fields: [
        {
          name: "json_path",
          label: "JSON path",
          control: "text",
          required: true,
          default: "",
          placeholder: "$.body",
          required_message: "Enter the JSONPath to the array",
        },
        {
          name: "length",
          label: "Max length",
          control: "number",
          required: true,
          default: 0,
          placeholder: "0",
          required_message: "Enter the maximum array length",
        },
      ],
      output: { branches: [{ id: "out", label: "Next" }] },
    },
    {
      kind: "add_attribute",
      label: "Add Attribute",
      category: "json",
      applies_to: "json",
      node_kind: "expression",
      summary:
        "Sets (upserts) a value at a JSON path, creating missing parents; replaces if present.",
      fields: [
        {
          name: "json_path",
          label: "JSON path",
          control: "text",
          required: true,
          default: "",
          placeholder: "$.paywall_show",
          required_message: "Enter the JSONPath to set",
        },
        {
          name: "value",
          label: "Value",
          control: "text",
          required: true,
          default: "",
          placeholder: "<html>…</html>",
          required_message: "Enter the value to set",
        },
      ],
      output: { branches: [{ id: "out", label: "Next" }] },
    },
    {
      kind: "apply_outcome",
      label: "Apply",
      category: "content",
      applies_to: "all",
      node_kind: "expression",
      summary: "Applies a saved outcome's components to the body, then continues.",
      fields: [
        {
          name: "outcome_id",
          label: "Outcome",
          control: "outcome_select",
          required: true,
          default: "",
          required_message: "Pick an outcome to apply",
        },
      ],
      output: { branches: [{ id: "out", label: "Next" }] },
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
