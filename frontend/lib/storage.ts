// lib/storage.ts — състояние синхронизирано с локалСторидж през
// юзСинкЕкстърналСтор (без сетСтейт в ефект; хидратационно-безопасно).

import { useSyncExternalStore } from "react";

const CHANGE_EVENT = "bagabuch-storage-change";

export function subscribeStorage(callback: () => void): () => void {
  return subscribe(callback);
}

function subscribe(callback: () => void): () => void {
  if (typeof window === "undefined") return () => {};
  const handler = () => callback();
  window.addEventListener("storage", handler);
  window.addEventListener(CHANGE_EVENT, handler);
  return () => {
    window.removeEventListener("storage", handler);
    window.removeEventListener(CHANGE_EVENT, handler);
  };
}

export function readStorage(key: string): string | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(key);
}

export function writeStorage(key: string, value: string | null): void {
  if (typeof window === "undefined") return;
  if (value === null) window.localStorage.removeItem(key);
  else window.localStorage.setItem(key, value);
  window.dispatchEvent(new Event(CHANGE_EVENT));
}

// Връща стойността от локалСторидж; на сървъра — сървърния снапшот.
export function useStorageValue(key: string, serverFallback: string): string {
  return useSyncExternalStore(
    subscribe,
    () => readStorage(key) ?? serverFallback,
    () => serverFallback
  );
}
