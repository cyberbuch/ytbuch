"use client";

import { useMemo, useState } from "react";
import { UnitOfMeasure, filterUnits, unitOptionLabel } from "../lib/units";
import { useI18n } from "./I18nProvider";

interface Props {
  value: string;
  onChange: (code: string) => void;
  units: UnitOfMeasure[];
  compact?: boolean;
}

export function UnitPicker({ value, onChange, units, compact }: Props) {
  const { t } = useI18n();
  const [q, setQ] = useState("");
  const extra = useMemo(() => {
    if (value && !units.some((u) => u.code === value)) {
      return [
        {
          code: value,
          name: value,
          name_en: "",
          symbol: value,
          category: "",
          source: "",
        },
      ];
    }
    return [];
  }, [value, units]);
  const filtered = extra.concat(compact ? units : filterUnits(units, q));

  if (compact) {
    return (
      <select
        className="select"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        {filtered.map((u) => (
          <option key={u.code} value={u.code}>
            {u.symbol ? `${u.symbol} (${u.code})` : u.code}
          </option>
        ))}
      </select>
    );
  }

  return (
    <div className="unit-picker">
      <input
        className="input"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        placeholder={t("products.unit_search")}
      />
      <select
        className="select"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        size={8}
      >
        {filtered.map((u) => (
          <option key={u.code} value={u.code}>
            {unitOptionLabel(u)}
          </option>
        ))}
      </select>
    </div>
  );
}
