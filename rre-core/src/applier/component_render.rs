//! Component template rendering (Component Editor, design §4.2).
//!
//! Renders a mustache `html_body` against the rule node's flat `variables` map.
//! Mustache scope here is FLAT interpolation only (no sections / partials /
//! lambdas), matching the flat variable model:
//!
//! - `{{x}}` — HTML-escaped (mustache default).
//! - `{{{x}}}` / `{{&x}}` — raw (unescaped) — the rendered output is `ammonia`-
//!   sanitized by the caller before injection, so raw values still get cleaned.
//! - A variable absent from `values` renders to the empty string (mustache's
//!   missing-key default), never a panic.
//!
//! Values are coerced to strings: a JSON string is used verbatim; any non-string
//! scalar (number / bool) is stringified; `null` and structured values (array /
//! object) render empty (the flat model only carries scalar variable values).
//! A malformed template (unbalanced `{{ }}`) → `Err(ApplyError::Render)` so the
//! caller fails OPEN (component skipped, body untouched).

use mustache::MapBuilder;
use serde_json::{Map, Value};

use crate::applier::ApplyError;

/// Render `html_body` (a mustache template) against the flat `values` map.
///
/// Pure + deterministic: the same `(html_body, values)` always yields the same
/// output, so running it twice (idempotency) is identical. Returns
/// `Err(ApplyError::Render)` only when the template itself fails to compile;
/// missing variables are NOT an error (they render empty).
pub fn render(html_body: &str, values: &Map<String, Value>) -> Result<String, ApplyError> {
    let template = mustache::compile_str(html_body).map_err(|_| ApplyError::Render)?;

    let mut builder = MapBuilder::new();
    for (name, value) in values {
        builder = builder.insert_str(name.clone(), coerce_str(value));
    }
    let data = builder.build();

    template
        .render_data_to_string(&data)
        .map_err(|_| ApplyError::Render)
}

/// Coerce a JSON variable value to the string mustache renders. Strings pass
/// through verbatim; numbers / bools are stringified; `null` and structured
/// values (array / object) render to the empty string (flat scalar model only).
fn coerce_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        // null / array / object: empty (flat model carries scalars only).
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn values(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn escapes_double_brace_interpolation() {
        let out = render(
            "<p>{{headline}}</p>",
            &values(json!({ "headline": "Tom & <b>Jerry</b>" })),
        )
        .unwrap();
        assert_eq!(out, "<p>Tom &amp; &lt;b&gt;Jerry&lt;/b&gt;</p>");
    }

    #[test]
    fn triple_brace_is_raw() {
        let out = render(
            "<p>{{{headline}}}</p>",
            &values(json!({ "headline": "<b>Jerry</b>" })),
        )
        .unwrap();
        assert_eq!(out, "<p><b>Jerry</b></p>");
    }

    #[test]
    fn ampersand_sigil_is_raw() {
        let out = render(
            "<p>{{&headline}}</p>",
            &values(json!({ "headline": "<i>x</i>" })),
        )
        .unwrap();
        assert_eq!(out, "<p><i>x</i></p>");
    }

    #[test]
    fn missing_variable_renders_empty() {
        let out = render("<p>{{absent}}!</p>", &values(json!({}))).unwrap();
        assert_eq!(out, "<p>!</p>");
    }

    #[test]
    fn non_string_values_are_coerced() {
        let out = render("<p>{{n}}/{{b}}</p>", &values(json!({ "n": 42, "b": true }))).unwrap();
        assert_eq!(out, "<p>42/true</p>");
    }

    #[test]
    fn null_and_structured_render_empty() {
        let out = render(
            "[{{a}}][{{b}}][{{c}}]",
            &values(json!({ "a": null, "b": [1, 2], "c": { "k": "v" } })),
        )
        .unwrap();
        assert_eq!(out, "[][][]");
    }

    #[test]
    fn idempotent_same_input_same_output() {
        let body = "<a href=\"{{url}}\">{{label}}</a>";
        let vals = values(json!({ "url": "https://x", "label": "Go" }));
        let a = render(body, &vals).unwrap();
        let b = render(body, &vals).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn malformed_template_errors() {
        // Unbalanced `{{` → compile error → fail-open at the caller.
        let err = render("<p>{{unclosed</p>", &values(json!({})));
        assert!(matches!(err, Err(ApplyError::Render)));
    }
}
