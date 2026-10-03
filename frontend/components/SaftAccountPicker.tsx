"use client";

// Търсачка към номенклатурата NRA_Nom_Accounts (SAF-T сметкоплан на НАП).

import { useMemo, useState } from "react";
import { useI18n } from "./I18nProvider";

export interface SaftNomAccount {
  code: string;
  name: string;
  section?: string;
  section_name?: string;
  group?: string;
  group_name?: string;
}

interface Props {
  value: string;
  accounts: SaftNomAccount[];
  onChange: (code: string, acc: SaftNomAccount | null) => void;
}

export function SaftAccountPicker({ value, accounts, onChange }: Props) {
  const { t } = useI18n();
  const [q, setQ] = useState("");

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    const extra: SaftNomAccount[] =
      value && !accounts.some((a) => a.code === value)
        ? [{ code: value, name: value }]
        : [];
    const src = extra.concat(accounts);
    if (!s) return src.slice(0, 80);
    return src
      .filter((a) =>
        `${a.code} ${a.name} ${a.section_name ?? ""} ${a.group_name ?? ""}`
          .toLowerCase()
          .includes(s)
      )
      .slice(0, 80);
  }, [accounts, q, value]);

  const selected = accounts.find((a) => a.code === value);

  return (
    <div className="unit-picker">
      <input
        className="input"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        placeholder={t("accounts.saft_search")}
      />
      <select
        className="select"
        value={value}
        size={8}
        onChange={(e) => {
          const code = e.target.value;
          const acc = accounts.find((a) => a.code === code) ?? null;
          onChange(code, acc);
        }}
      >
        <option value="">{t("accounts.saft_none")}</option>
        {filtered.map((a) => (
          <option key={a.code} value={a.code}>
            {a.code} — {a.name}
          </option>
        ))}
      </select>
      {selected && (
        <div className="muted" style={{ fontSize: 12 }}>
          {selected.section ? `${selected.section} ${selected.section_name}` : ""}
          {selected.group ? ` · ${selected.group} ${selected.group_name}` : ""}
        </div>
      )}
    </div>
  );
}
