"use client";

import { useEffect, useMemo, useState } from "react";
import { api, ListResponse } from "../lib/api";
import { useI18n } from "./I18nProvider";

export interface Cn8Item {
  code: string;
  name: string;
  raw_name?: string;
  heading?: string;
  heading_name?: string;
  chapter?: string;
  chapter_name?: string;
}

interface Props {
  code: string;
  name: string;
  candidates?: Cn8Item[];
  onSelect: (code: string, name: string) => void;
}

export function Cn8Picker({ code, name, candidates = [], onSelect }: Props) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [found, setFound] = useState<Cn8Item[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    const q = query.trim();
    let active = true;
    // При празна търсачка се показват първите кодове от каталога, за да не е
    // списъкът празен при отваряне на формата; писането го стеснява.
    const timer = window.setTimeout(async () => {
      setLoading(true);
      try {
        const data = await api.get<ListResponse<Cn8Item>>("/v1/saft/nomenclatures", {
          kind: "cn8",
          search: q || undefined,
          limit: 40,
        });
        if (active) {
          setFound(data.items ?? []);
          setError("");
        }
      } catch (e) {
        if (active) setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (active) setLoading(false);
      }
    }, 250);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [query]);

  const items = useMemo(() => {
    const map = new Map<string, Cn8Item>();
    for (const item of [...candidates, ...found]) map.set(item.code, item);
    if (code && !map.has(code)) {
      map.set(code, { code, name: name || code });
    }
    return [...map.values()];
  }, [candidates, found, code, name]);

  return (
    <div>
      <input
        className="input"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder={t("products.cn8_search")}
      />
      <select
        className="select"
        style={{ marginTop: 6 }}
        size={6}
        value={code}
        onChange={(e) => {
          const item = items.find((x) => x.code === e.target.value);
          if (item) onSelect(item.code, item.name);
        }}
      >
        {!code && <option value="">{t("products.cn8_pick")}</option>}
        {items.map((item) => (
          <option key={item.code} value={item.code}>
            {item.code} — {item.name}
          </option>
        ))}
      </select>
      {loading && <div className="muted">{t("common.loading")}</div>}
      {error && <div className="error-text">{error}</div>}
      {code && (
        <div className="muted" style={{ marginTop: 6, fontSize: 12 }}>
          <strong>{code}</strong> — {name}
        </div>
      )}
    </div>
  );
}
