"use client";

// Търсачка за сметка: поле с номер/име, под него списък със съвпадения —
// същият модел като Cn8Picker и SaftAccountPicker. Полето остава свободен
// текст, за да може да се въведе и префикс (напр. 60201), който не е
// задължително да съществува като сметка.

import { useMemo } from "react";

export interface AccountItem {
  number: string;
  name: string;
}

interface Props {
  value: string;
  accounts: AccountItem[];
  onChange: (number: string) => void;
  placeholder?: string;
}

export function AccountPicker({ value, accounts, onChange, placeholder }: Props) {
  const filtered = useMemo(() => {
    const s = value.trim().toLowerCase();
    const list = s
      ? accounts.filter((a) => `${a.number} ${a.name}`.toLowerCase().includes(s))
      : accounts;
    return list.slice(0, 60);
  }, [accounts, value]);

  return (
    <div className="unit-picker">
      <input
        className="input"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
      <select
        className="select"
        size={6}
        value={accounts.some((a) => a.number === value) ? value : ""}
        onChange={(e) => onChange(e.target.value)}
      >
        {filtered.map((a) => (
          <option key={a.number} value={a.number}>
            {a.number} — {a.name}
          </option>
        ))}
      </select>
    </div>
  );
}
