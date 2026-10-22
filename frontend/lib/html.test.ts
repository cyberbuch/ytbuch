import { describe, it, expect } from "vitest";
import { escapeHtml } from "./html";
import { absorbVatIntoLines, calcLine, calcTotals, emptyLine, isCompanyVatRegistered } from "./invoice";

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

describe("isCompanyVatRegistered", () => {
  it("requires both is_vat_registered=1 and a non-empty vat_number", () => {
    expect(isCompanyVatRegistered({ is_vat_registered: 1, vat_number: "BG123456789" })).toBe(true);
    expect(isCompanyVatRegistered({ is_vat_registered: 0, vat_number: "BG123456789" })).toBe(false);
    expect(isCompanyVatRegistered({ is_vat_registered: 1, vat_number: "" })).toBe(false);
    expect(isCompanyVatRegistered({ is_vat_registered: 1, vat_number: "   " })).toBe(false);
  });
});

describe("absorbVatIntoLines", () => {
  it("absorbs 20% VAT into line amounts without rounding drift", () => {
    const lines = [
      {
        ...emptyLine(),
        description: "Артикул 1",
        quantity: "3",
        unit_price: "3.33",
        vat_rate: "20",
        net_amount: "9.99",
        vat_amount: "2.00",
        total_amount: "11.99",
      },
      {
        ...emptyLine(),
        description: "Артикул 2",
        quantity: "1",
        unit_price: "10.00",
        vat_rate: "20",
        net_amount: "10.00",
        vat_amount: "2.00",
        total_amount: "12.00",
      },
    ];
    const absorbed = absorbVatIntoLines(lines, "23.99");
    const priced = absorbed.map((l) => calcLine(l, false, false));
    const totals = calcTotals(priced, "0");
    expect(totals.subtotal).toBe("23.99");
    expect(totals.vat).toBe("0.00");
    expect(totals.total).toBe("23.99");
    expect(priced[0].net_amount).toBe("11.99");
    expect(priced[0].total_amount).toBe("11.99");
    expect(priced[0].vat_rate).toBe("0");
  });
});
