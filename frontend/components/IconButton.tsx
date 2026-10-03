"use client";

import Link from "next/link";
import { MouseEvent, ReactNode } from "react";

export type IconName =
  | "edit"
  | "delete"
  | "view"
  | "print"
  | "activate"
  | "history"
  | "action"
  | "confirm";

const ICONS: Record<IconName, ReactNode> = {
  edit: (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path
        fill="currentColor"
        d="M11.7 1.3a1.5 1.5 0 0 1 2.1 2.1l-.4.4-2.1-2.1.4-.4ZM3 10.4 9.7 3.7l2.1 2.1L5.1 12.5H3v-2.1Z"
      />
    </svg>
  ),
  delete: (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path
        fill="currentColor"
        d="M6 2h4l.5 1H14v1H2V3h3.5L6 2Zm1 4v6H6V6h1Zm3 0v6H9V6h1ZM4.5 5h7l-.6 8.1a1 1 0 0 1-1 .9H6.1a1 1 0 0 1-1-.9L4.5 5Z"
      />
    </svg>
  ),
  view: (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path
        fill="currentColor"
        d="M8 3c3.5 0 6.4 2.4 7.4 5-1 2.6-3.9 5-7.4 5S1.6 10.6.6 8C1.6 5.4 4.5 3 8 3Zm0 2.2A2.8 2.8 0 1 0 8 10.8 2.8 2.8 0 0 0 8 5.2Z"
      />
    </svg>
  ),
  print: (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path
        fill="currentColor"
        d="M4 2h8v3H4V2Zm-2 4h12a1 1 0 0 1 1 1v5h-3v3H4v-3H1V7a1 1 0 0 1 1-1Zm3 6v2h6v-2H5Zm8-4.5a.8.8 0 1 0 0 1.6.8.8 0 0 0 0-1.6Z"
      />
    </svg>
  ),
  activate: (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path
        fill="currentColor"
        d="M8 1.5a6.5 6.5 0 1 1 0 13 6.5 6.5 0 0 1 0-13Zm-.2 8.6 4-4-.9-.9-3.1 3.1-1.5-1.5-.9.9 2.4 2.4Z"
      />
    </svg>
  ),
  history: (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path
        fill="currentColor"
        d="M8 1.5a6.5 6.5 0 1 1-6.1 4.3l1.3.4A5.2 5.2 0 1 0 4 4.3V6H2.5V2.8H6v1.3H4.7A6.5 6.5 0 0 1 8 1.5ZM7.2 5h1.3v3.2l2.3 1.4-.7 1.1-2.9-1.8V5Z"
      />
    </svg>
  ),
  action: (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path
        fill="currentColor"
        d="M13.2 4.2 11.8 2.8a1 1 0 0 0-1.4 0L9.6 3.6l2.8 2.8.8-.8a1 1 0 0 0 0-1.4ZM3.2 12.2l1.8-1.8 2.8 2.8-1.8 1.8H3.2v-2.8Zm5.4-6.1L4.3 10.4l1.3 1.3 4.3-4.3-1.3-1.3Z"
      />
    </svg>
  ),
  confirm: (
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path
        fill="currentColor"
        d="M2 8.2 6.2 12.5 14 3.8l-1.2-1-6.5 7.3-3-3.1L2 8.2Z"
      />
    </svg>
  ),
};

export function IconButton({
  icon,
  title,
  onClick,
  danger,
  disabled,
  href,
}: {
  icon: IconName;
  title: string;
  onClick?: (e: MouseEvent<HTMLButtonElement>) => void;
  danger?: boolean;
  disabled?: boolean;
  href?: string;
}) {
  const cls = `btn btn-icon${danger ? " btn-danger" : ""}`;
  const glyph = ICONS[icon];
  if (href) {
    return (
      <Link href={href} className={cls} title={title} aria-label={title}>
        {glyph}
      </Link>
    );
  }
  return (
    <button
      type="button"
      className={cls}
      title={title}
      aria-label={title}
      onClick={onClick}
      disabled={disabled}
    >
      {glyph}
    </button>
  );
}
