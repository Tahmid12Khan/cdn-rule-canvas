import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ComponentPreview } from "@/components/component-library/ComponentPreview";

// The preview renders into a srcdoc <iframe> so component scripts, remote CSS
// and custom elements behave as they do on the real site. jsdom doesn't parse
// srcdoc, so these assert on the document handed to the frame.
function srcdoc() {
  return screen.getByTitle("Component preview").getAttribute("srcdoc") ?? "";
}

describe("ComponentPreview", () => {
  it("renders interpolated mustache values", () => {
    render(
      <ComponentPreview
        html="<h1>{{headline}}</h1><p>{{body}}</p>"
        values={{ headline: "Subscribe now", body: "Get full access." }}
      />,
    );
    expect(srcdoc()).toContain("<h1>Subscribe now</h1>");
    expect(srcdoc()).toContain("<p>Get full access.</p>");
  });

  it("renders missing variables as empty", () => {
    render(<ComponentPreview html="<p>{{absent}}!</p>" values={{}} />);
    expect(srcdoc()).toContain("<p>!</p>");
  });

  it("escapes interpolated values (double-brace) so they stay text", () => {
    render(
      <ComponentPreview
        html="<div>{{name}}</div>"
        values={{ name: "<script>alert(1)</script>" }}
      />,
    );
    // Mustache escapes the value, so an author-entered tag stays text and never
    // becomes a live element inside the frame.
    expect(srcdoc()).toContain("&lt;script&gt;alert(1)&lt;");
    expect(srcdoc()).not.toContain("<script>alert(1)</script>");
  });

  it("passes the component's own scripts through unmodified", () => {
    render(
      <ComponentPreview
        html='<div id="host"></div><script src="https://cdn.example/app.js"></script>'
        values={{}}
      />,
    );
    expect(srcdoc()).toContain('<script src="https://cdn.example/app.js">');
  });

  it("falls back to the raw body on a malformed template instead of crashing", () => {
    render(<ComponentPreview html="<p>{{unclosed</p>" values={{}} />);
    expect(srcdoc()).toContain("{{unclosed");
  });

  it("sandboxes the frame but allows scripts", () => {
    render(<ComponentPreview html="<p>hi</p>" values={{}} />);
    const sandbox =
      screen.getByTitle("Component preview").getAttribute("sandbox") ?? "";
    expect(sandbox).toContain("allow-scripts");
    expect(sandbox).not.toContain("allow-top-navigation");
  });
});
