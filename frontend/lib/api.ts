// lib/api.ts — HTTP клиент към bagabuch backend-а (:8080) с JWT.

import { readStorage, writeStorage } from "./storage";

export const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "";

export const TOKEN_KEY = "***";

export function getToken(): string | null {
  return readStorage(TOKEN_KEY);
}

export function setToken(token: string | null): void {
  writeStorage(TOKEN_KEY, token);
}

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

function formatApiDetail(data: unknown, fallback: string): string {
  if (typeof data === "string" && data.length > 0) return data;
  if (data === null || data === undefined || typeof data !== "object") return fallback;
  const detail = (data as { detail?: unknown }).detail;
  if (typeof detail === "string" && detail.length > 0) return detail;
  if (Array.isArray(detail)) {
    const parts: string[] = [];
    for (const item of detail) {
      if (typeof item === "string") {
        parts.push(item);
        continue;
      }
      if (item && typeof item === "object") {
        const o = item as { msg?: unknown; loc?: unknown };
        const msg = typeof o.msg === "string" ? o.msg : JSON.stringify(item);
        if (Array.isArray(o.loc) && o.loc.length > 0) {
          const loc = o.loc.filter((x) => typeof x === "string").join(".");
          parts.push(loc ? `${loc}: ${msg}` : msg);
        } else {
          parts.push(msg);
        }
      }
    }
    if (parts.length > 0) return parts.join("; ");
  }
  if (detail !== undefined) {
    try {
      return JSON.stringify(detail);
    } catch {
      return fallback;
    }
  }
  return fallback;
}

async function request<T>(
  path: string,
  method: string,
  body?: unknown
): Promise<T> {
  const headers: Record<string, string> = {};
  const token = getToken();
  if (token) headers["Authorization"] = `Bearer ${token}`;
  if (body !== undefined) headers["Content-Type"] = "application/json";
  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      cache: "no-store",
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg === "Failed to fetch" || msg === "Load failed" || msg === "NetworkError when attempting to fetch resource.") {
      throw new ApiError(
        0,
        "Връзката със сървъра се скъса. Рестартирай backend-а (./scripts/stop.sh && ./scripts/dev.sh) и опитай пак."
      );
    }
    throw e;
  }
  if (res.status === 204) return undefined as T;
  let data: unknown = null;
  const text = await res.text();
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = text;
    }
  }
  if (!res.ok) {
    throw new ApiError(res.status, formatApiDetail(data, res.statusText));
  }
  return data as T;
}

export const api = {
  get: <T>(path: string) => request<T>(path, "GET"),
  post: <T>(path: string, body?: unknown) => request<T>(path, "POST", body),
  put: <T>(path: string, body?: unknown) => request<T>(path, "PUT", body),
  patch: <T>(path: string, body?: unknown) => request<T>(path, "PATCH", body),
  del: <T>(path: string) => request<T>(path, "DELETE"),
};

export async function downloadFile(path: string, filename: string): Promise<void> {
  const headers: Record<string, string> = {};
  const token = getToken();
  if (token) headers["Authorization"] = `Bearer ${token}`;
  const res = await fetch(`${API_BASE}${path}`, { headers, cache: "no-store" });
  if (!res.ok) {
    const text = await res.text();
    let parsed: unknown = text;
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = text;
    }
    throw new ApiError(res.status, formatApiDetail(parsed, res.statusText || text));
  }
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export type TokenResponse = {
  access_token?: string;
  mfa?: string;
  mfa_token?: string;
  message?: string;
  expires_in?: number;
};

export async function login(
  username: string,
  password: string
): Promise<TokenResponse> {
  const data = await api.post<TokenResponse>("/v1/auth/token", {
    sub: username,
    email: username,
    password,
  });
  if (data.access_token) setToken(data.access_token);
  return data;
}

export async function loginMfa(mfaToken: string, code: string): Promise<TokenResponse> {
  const data = await api.post<TokenResponse>("/v1/auth/mfa", {
    mfa_token: mfaToken,
    code,
  });
  if (data.access_token) setToken(data.access_token);
  return data;
}

export function logout(): void {
  setToken(null);
}

// общ тип за списък
export interface ListResponse<T> {
  items: T[];
  count: number;
}

// фирма
export interface Company {
  id: number;
  name: string;
  eik: string;
  [key: string]: unknown;
}

// активна фирма (мултитенант)
// Събитие, на което хедърът слуша, за да обнови активната фирма.
export const ACTIVE_COMPANY_EVENT = "bagabuch-active-company-changed";

export function getActiveCompany(): Promise<Company | Record<string, never>> {
  return api.get<Company | Record<string, never>>("/v1/active-company");
}

export async function setActiveCompany(id: number): Promise<Company> {
  const data = await api.put<Company>("/v1/active-company", {
    company_id: id,
  });
  if (typeof window !== "undefined") {
    window.dispatchEvent(new Event(ACTIVE_COMPANY_EVENT));
  }
  return data;
}
