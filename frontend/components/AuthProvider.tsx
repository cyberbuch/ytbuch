"use client";

// AuthProvider — JWT сесия. Токенът се чете реактивно от локалСторидж.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  useSyncExternalStore,
  ReactNode,
} from "react";
import {
  TOKEN_KEY,
  login as apiLogin,
  logout as apiLogout,
  getMe,
} from "../lib/api";
import { subscribeStorage, readStorage } from "../lib/storage";

interface AuthContextValue {
  authed: boolean;
  ready: boolean;
  permissions: string[];
  can: (permission: string) => boolean;
  login: (username: string, password: string) => Promise<{ mfa?: string; mfa_token?: string }>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextValue>({
  authed: false,
  ready: false,
  permissions: [],
  can: () => true,
  login: async () => ({}),
  logout: () => {},
});

// Монтиран флаг: фалш на сървъра/хидратацията, истина на клиента —
// без сетСтейт в ефект (юзСинкЕкстърналСтор с празен стор).
function useMounted(): boolean {
  return useSyncExternalStore(
    () => () => {},
    () => true,
    () => false
  );
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const ready = useMounted();
  const token = useSyncExternalStore(
    subscribeStorage,
    () => readStorage(TOKEN_KEY) ?? "",
    () => ""
  );
  const authed = token !== "";

  // Правата идват от /v1/me при всяка смяна на токена (вход, изход).
  const [permissions, setPermissions] = useState<string[]>([]);
  useEffect(() => {
    if (token === "") {
      setPermissions([]);
      return;
    }
    let alive = true;
    getMe()
      .then((me) => {
        if (alive) setPermissions(me.permissions ?? []);
      })
      .catch(() => {
        if (alive) setPermissions([]);
      });
    return () => {
      alive = false;
    };
  }, [token]);

  const can = useCallback(
    (permission: string) =>
      permissions.includes("*") || permissions.includes(permission),
    [permissions]
  );

  const login = useCallback(async (username: string, password: string) => {
    const r = await apiLogin(username, password);
    return { mfa: r.mfa, mfa_token: r.mfa_token };
  }, []);

  const logout = useCallback(() => {
    apiLogout();
  }, []);

  return (
    <AuthContext.Provider value={{ authed, ready, permissions, can, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  return useContext(AuthContext);
}
