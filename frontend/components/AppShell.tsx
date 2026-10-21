"use client";

// AppShell — обвивка за автентикираните страници: странична навигация +
// горна лента (активна фирма + превключвател, смяна на тема, език, изход).

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { ReactNode, useCallback, useEffect, useState } from "react";
import { useI18n } from "./I18nProvider";
import { useTheme } from "./ThemeProvider";
import { useAuth } from "./AuthProvider";
import {
  ACTIVE_COMPANY_EVENT,
  Company,
  ListResponse,
  api,
  getActiveCompany,
  setActiveCompany,
} from "../lib/api";

// Менюто се скрива без съответното право (permission). Празен perm е
// винаги видим: табло, каталози, собствен профил. Ключовете са тези,
// които backend-ът проверява в gate.rs.
const NAV_ITEMS = [
  { href: "/", key: "nav.dashboard" },
  { href: "/companies", key: "nav.companies", perm: "companies" },
  { href: "/settings", key: "nav.settings", perm: "companies" },
  { href: "/accounts", key: "nav.accounts", perm: "accounts" },
  { href: "/counterparts", key: "nav.counterparts", perm: "counterparts" },
  { href: "/products", key: "nav.products", perm: "products" },
  { href: "/bank-accounts", key: "nav.bank_accounts", perm: "banking" },
  { href: "/bank-transactions", key: "nav.bank_transactions", perm: "banking" },
  { href: "/invoices", key: "nav.invoices", perm: "invoices" },
  { href: "/scan", key: "nav.scan", perm: "scans" },
  { href: "/internal-docs", key: "nav.internal_docs", perm: "stock" },
  { href: "/material-issues", key: "nav.material_issues", perm: "stock" },
  { href: "/stock", key: "nav.stock", perm: "stock" },
  { href: "/production", key: "nav.production", perm: "production" },
  { href: "/journal", key: "nav.journal", perm: "journal" },
  { href: "/reports", key: "nav.reports", perm: "reports" },
  { href: "/audit", key: "nav.audit", perm: "reports" },
  { href: "/opening-balances", key: "nav.opening_balances", perm: "periods" },
  { href: "/accounting-periods", key: "nav.periods", perm: "periods" },
  { href: "/vat", key: "nav.vat", perm: "vat" },
  { href: "/dividends", key: "nav.dividends", perm: "dividends" },
  { href: "/exchange-rates", key: "nav.exchange_rates", perm: "rates" },
  { href: "/saft", key: "nav.saft", perm: "saft" },
  { href: "/saft-movement-mappings", key: "nav.saft_mappings", perm: "saft" },
  { href: "/nomenclatures", key: "nav.nomenclatures" },
  { href: "/fixed-assets", key: "nav.fixed_assets", perm: "assets" },
  { href: "/admin", key: "nav.admin", perm: "settings" },
  { href: "/profile", key: "nav.profile" },
  { href: "/users", key: "nav.users", perm: "users", superOnly: true },
  { href: "/roles", key: "nav.roles", perm: "users", superOnly: true },
];

export function AppShell({ children }: { children: ReactNode }) {
  const { t, lang, setLang } = useI18n();
  const { theme, toggleTheme } = useTheme();
  const { logout, can, isSuperAdmin } = useAuth();
  const pathname = usePathname();
  const router = useRouter();

  const [companies, setCompanies] = useState<Company[]>([]);
  const [activeId, setActiveId] = useState<number>(0);

  const refreshActive = useCallback(async () => {
    try {
      const ac = await getActiveCompany();
      setActiveId(ac && typeof ac.id === "number" ? ac.id : 0);
    } catch {
      setActiveId(0);
    }
  }, []);

  const loadCompanies = useCallback(async () => {
    try {
      const data = await api.get<ListResponse<Company>>("/v1/companies");
      setCompanies(data.items ?? []);
    } catch {
      setCompanies([]);
    }
  }, []);

  // Зареждане при монтиране + слушане за смяна на активната фирма.
  // данни-фектчинг: сетСтейт е асинхронен след await.
  useEffect(() => {
    loadCompanies();
    refreshActive();
    const onActiveChange = () => refreshActive();
    window.addEventListener(ACTIVE_COMPANY_EVENT, onActiveChange);
    return () =>
      window.removeEventListener(ACTIVE_COMPANY_EVENT, onActiveChange);
  }, [loadCompanies, refreshActive]);

  const handleLogout = () => {
    logout();
    router.push("/login");
  };

  const handleSwitch = async (id: number) => {
    if (id === 0) return;
    try {
      await setActiveCompany(id);
      setActiveId(id);
      window.location.reload();
    } catch {
      /* запази текущата */
    }
  };

  const activeCompany = companies.find((c) => c.id === activeId);

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="sidebar-brand">{t("app.name")}</div>
        <nav className="sidebar-nav">
          {NAV_ITEMS.filter((item) => (!item.perm || can(item.perm)) && (!item.superOnly || isSuperAdmin)).map((item) => {
            const active =
              item.href === "/"
                ? pathname === "/"
                : pathname.startsWith(item.href);
            return (
              <Link
                key={item.href}
                href={item.href}
                className={`nav-link${active ? " nav-link-active" : ""}`}
              >
                {t(item.key)}
              </Link>
            );
          })}
        </nav>
      </aside>
      <div className="shell-main">
        <header className="topbar">
          <div className="topbar-company">
            <span className="muted">{t("active.label")}:</span>
            <select
              className="select company-switch"
              value={activeId}
              onChange={(e) => handleSwitch(Number(e.target.value))}
            >
              <option value={0}>{t("active.none")}</option>
              {companies.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
            {activeCompany && (
              <span className="badge badge-success">{activeCompany.eik}</span>
            )}
            <Link href="/companies" className="btn btn-ghost btn-sm">
              {t("nav.companies")}
            </Link>
          </div>
          <div className="topbar-actions">
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => setLang(lang === "bg" ? "en" : "bg")}
              title={lang === "bg" ? "English" : "Български"}
            >
              {lang === "bg" ? "EN" : "BG"}
            </button>
            <button
              className="btn btn-ghost btn-sm"
              onClick={toggleTheme}
              title={theme === "dark" ? "Light" : "Dark"}
            >
              {theme === "dark" ? "☀" : "☾"}
            </button>
            <Link href="/profile" className="btn btn-ghost btn-sm">
              {t("nav.profile")}
            </Link>
            <button className="btn btn-ghost btn-sm" onClick={handleLogout}>
              {t("nav.logout")}
            </button>
          </div>
        </header>
        <main className="content">{children}</main>
      </div>
    </div>
  );
}
