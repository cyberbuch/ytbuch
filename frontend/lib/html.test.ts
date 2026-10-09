import { describe, it, expect } from "vitest";
import { escapeHtml } from "./html";

describe("escapeHtml", () => {
  it("escapes HTML special characters", () => {
    expect(escapeHtml(`<script>alert("xss")</script>`)).toBe(
      "&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;"
    );
  });

  it("escapes ampersand, quotes and apostrophes", () => {
    expect(escapeHtml(`a & b 'c' "d"`)).toBe("a &amp; b &#39;c&#39; &quot;d&quot;");
  });

  it("returns empty string for null and undefined", () => {
    expect(escapeHtml(null)).toBe("");
    expect(escapeHtml(undefined)).toBe("");
  });

  it("converts numbers to strings", () => {
    expect(escapeHtml(42)).toBe("42");
    expect(escapeHtml(0)).toBe("0");
  });

  it("leaves safe text unchanged", () => {
    expect(escapeHtml("Здравей, свят!")).toBe("Здравей, свят!");
  });
});
