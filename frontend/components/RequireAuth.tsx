"use client";

// RequireAuth — пази автентикираните страници; пренасочва към /вход без токен.

import { ReactNode, useEffect } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "./AuthProvider";
import { useI18n } from "./I18nProvider";
import { AppShell } from "./AppShell";

export function RequireAuth({ children }: { children: ReactNode }) {
  const { authed, ready } = useAuth();
  const { t } = useI18n();
  const router = useRouter();

  useEffect(() => {
    if (ready && !authed) router.replace("/login");
  }, [ready, authed, router]);

  if (!ready) {
    return <div className="content muted">{t("common.loading")}</div>;
  }
  if (!authed) {
    return null;
  }
  return <AppShell>{children}</AppShell>;
}
