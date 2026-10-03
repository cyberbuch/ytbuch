"use client";

import { InvoiceLine, VAT_RATES, emptyLine, num } from "../lib/invoice";
import { UnitOfMeasure } from "../lib/units";
import { UnitPicker } from "./UnitPicker";

export function InvoiceFormLines({
  t,
  pricedLines,
  pricesIncludeVat,
  setPricesIncludeVat,
  setLine,
  setLines,
  setItemOpenFor,
  setItemQuery,
  units,
}: {
  t: (k: string) => string;
  pricedLines: InvoiceLine[];
  pricesIncludeVat: boolean;
  setPricesIncludeVat: (v: boolean) => void;
  setLine: (i: number, patch: Partial<InvoiceLine>) => void;
  setLines: (fn: (prev: InvoiceLine[]) => InvoiceLine[]) => void;
  setItemOpenFor: (i: number) => void;
  setItemQuery: (v: string) => void;
  units: UnitOfMeasure[];
}) {
  return (
    <section className="card invoice-section">
        <div className="invoice-section-head">
          <h2 className="invoice-section-title">{t("invoices.lines")}</h2>
          <label className="check-inline">
            <input
              type="checkbox"
              checked={pricesIncludeVat}
              onChange={(e) => setPricesIncludeVat(e.target.checked)}
            />
            {t("invoices.prices_include_vat")}
          </label>
        </div>
        <div className="lines-wrap">
          <table className="table lines-table">
            <thead>
              <tr>
                <th>{t("invoices.item")}</th>
                <th>{t("invoices.qty")}</th>
                <th>{t("products.unit")}</th>
                <th>
                  {pricesIncludeVat
                    ? t("invoices.price_with_vat")
                    : t("invoices.unit_price")}
                </th>
                <th>{t("products.vat_rate")}</th>
                <th>{t("invoices.line_total")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {pricedLines.map((l, i) => (
                <tr key={i}>
                  <td>
                    <button
                      type="button"
                      className={`btn picker-trigger${l.description ? "" : " picker-empty"}`}
                      onClick={() => {
                        setItemOpenFor(i);
                        setItemQuery("");
                      }}
                    >
                      {l.description
                        ? `${l.code ? `${l.code} — ` : ""}${l.description}`
                        : t("invoices.pick_item")}
                    </button>
                    <input
                      className="input"
                      value={l.description}
                      onChange={(e) => setLine(i, { description: e.target.value })}
                      placeholder={t("journal.description")}
                    />
                    {num(l.vat_rate) === 0 && (
                      <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>
                        {t("invoices.zero_vat_hint")}
                      </div>
                    )}
                  </td>
                  <td>
                    <input
                      className="input"
                      value={l.quantity}
                      onChange={(e) => setLine(i, { quantity: e.target.value })}
                    />
                  </td>
                  <td>
                    <UnitPicker
                      compact
                      value={l.unit || "C62"}
                      onChange={(unit) => setLine(i, { unit })}
                      units={units}
                    />
                  </td>
                  <td>
                    <input
                      className="input"
                      value={l.unit_price}
                      onChange={(e) => setLine(i, { unit_price: e.target.value })}
                    />
                  </td>
                  <td>
                    <select
                      className="select"
                      value={l.vat_rate}
                      onChange={(e) => setLine(i, { vat_rate: e.target.value })}
                    >
                      {VAT_RATES.map((r) => (
                        <option key={r} value={r}>
                          {r}%
                        </option>
                      ))}
                      {l.vat_rate &&
                        !(VAT_RATES as readonly string[]).includes(l.vat_rate) && (
                          <option value={l.vat_rate}>{l.vat_rate}%</option>
                        )}
                    </select>
                  </td>
                  <td className="num">{l.net_amount}</td>
                  <td>
                    <button
                      type="button"
                      className="btn btn-danger btn-sm"
                      onClick={() =>
                        setLines((prev) =>
                          prev.length > 1 ? prev.filter((_, idx) => idx !== i) : prev
                        )
                      }
                    >
                      ✕
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <button
          type="button"
          className="btn btn-sm"
          onClick={() => setLines((prev) => [...prev, emptyLine()])}
        >
          + {t("invoices.add_line")}
        </button>
    </section>
  );
}
