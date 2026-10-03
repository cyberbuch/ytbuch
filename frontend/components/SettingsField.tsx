"use client";

import { ReactNode } from "react";

export function SettingsField({
  label,
  children,
  span,
}: {
  label: string;
  children: ReactNode;
  span?: number;
}) {
  return (
    <div className="field" style={span ? { gridColumn: `span ${span}` } : undefined}>
      <label className="label">{label}</label>
      {children}
    </div>
  );
}
