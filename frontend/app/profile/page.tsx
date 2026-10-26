"use client";

// Моят профил: име, парола, имейл и 2FA в табове.
// QR е картинка от API-то. Резервните кодове се свалят като текстов файл.

import { FormEvent, Suspense, useCallback, useEffect, useState } from "react";
import Image from "next/image";
import { useSearchParams } from "next/navigation";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { api, setToken } from "../../lib/api";

type Tab = "account" | "password" | "email" | "security";

interface Me {
  username?: string;
  email?: string;
  name?: string;
  totp_enabled?: number;
}

function tabOf(raw: string | null): Tab {
  if (raw === "password" || raw === "email" || raw === "security" || raw === "account") {
    return raw;
  }
  return "account";
}

function downloadCodes(codes: string[]) {
  const text = [
    "ytbuch резервни кодове",
    "Всеки код е за едно ползване. Пазят се извън сървъра.",
    "",
    ...codes,
    "",
  ].join("\n");
  const blob = new Blob([text], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = "ytbuch-recovery-codes.txt";
  a.click();
  URL.revokeObjectURL(url);
}

function ProfileInner() {
  const { t } = useI18n();
  const params = useSearchParams();
  const [tab, setTab] = useState<Tab>(tabOf(params.get("tab")));
  const [me, setMe] = useState<Me | null>(null);
  const [name, setName] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [email, setEmail] = useState("");
  const [emailPassword, setEmailPassword] = useState("");
  const [secret, setSecret] = useState("");
  const [qr, setQr] = useState("");
  const [codes, setCodes] = useState<string[]>([]);
  const [code, setCode] = useState("");
  const [disablePassword, setDisablePassword] = useState("");
  const [msg, setMsg] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");

  const load = useCallback(async () => {
    const r = await api.get<Me>("/v1/me");
    setMe(r);
    setName(r.name ?? "");
    setEmail(r.email ?? "");
  }, []);

  useEffect(() => {
    void load().catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, [load]);

  const saveName = async (e: FormEvent) => {
    e.preventDefault();
    setBusy("name");
    setError("");
    setMsg("");
    try {
      const r = await api.patch<{ message?: string }>("/v1/me", { name });
      setMsg(r.message || t("profile.saved"));
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const savePassword = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    setMsg("");
    if (newPassword.length < 8) {
      setError(t("profile.password_short"));
      return;
    }
    if (newPassword !== confirm) {
      setError(t("profile.password_mismatch"));
      return;
    }
    setBusy("password");
    try {
      const r = await api.post<{ message?: string; access_token?: string }>("/v1/me/password", {
        current_password: currentPassword,
        new_password: newPassword,
      });
      // Смяната на паролата приключва старите сесии — сърърът връща нов токен.
      if (r.access_token) setToken(r.access_token);
      setMsg(r.message || t("profile.password_ok"));
      setCurrentPassword("");
      setNewPassword("");
      setConfirm("");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const saveEmail = async (e: FormEvent) => {
    e.preventDefault();
    setBusy("email");
    setError("");
    setMsg("");
    try {
      const r = await api.post<{ message?: string; access_token?: string }>("/v1/me/email", {
        email,
        current_password: emailPassword,
      });
      if (r.access_token) setToken(r.access_token);
      setMsg(r.message || t("profile.email_ok"));
      setEmailPassword("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const start = async () => {
    setBusy("start");
    setError("");
    setMsg("");
    try {
      const r = await api.post<{
        secret: string;
        qr_svg?: string;
        recovery_codes?: string[];
        message?: string;
      }>("/v1/me/totp/start");
      setSecret(r.secret);
      setQr(r.qr_svg ?? "");
      setCodes(r.recovery_codes ?? []);
      setMsg(r.message || t("security.started"));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const confirmTotp = async (e: FormEvent) => {
    e.preventDefault();
    setBusy("confirm");
    setError("");
    try {
      const r = await api.post<{ message?: string }>("/v1/me/totp/confirm", { code });
      setMsg(r.message || t("security.enabled"));
      setSecret("");
      setQr("");
      setCodes([]);
      setCode("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const disable = async (e: FormEvent) => {
    e.preventDefault();
    setBusy("disable");
    setError("");
    try {
      const r = await api.post<{ message?: string }>("/v1/me/totp/disable", {
        password: disablePassword,
        code,
      });
      setMsg(r.message || t("security.disabled"));
      setDisablePassword("");
      setCode("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const on = me?.totp_enabled === 1;
  const tabs: Tab[] = ["account", "password", "email", "security"];

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("profile.title")}</h1>
      </div>
      <p className="muted" style={{ margin: "0 0 12px" }}>
        {t("profile.hint")}
      </p>
      <div className="tabs tabs-wrap">
        {tabs.map((k) => (
          <button
            key={k}
            type="button"
            className={`tab${tab === k ? " tab-active" : ""}`}
            onClick={() => {
              setTab(k);
              setError("");
              setMsg("");
            }}
          >
            {t(`profile.tab.${k}`)}
          </button>
        ))}
      </div>
      {error && <div className="error-text">{error}</div>}
      {msg && <p className="muted">{msg}</p>}

      {tab === "account" && (
        <form className="card content" onSubmit={saveName}>
          <div className="field">
            <label className="label">{t("profile.username")}</label>
            <input className="input" value={me?.username ?? ""} readOnly />
          </div>
          <p className="muted">{t("profile.username_hint")}</p>
          <div className="field">
            <label className="label">{t("profile.email")}</label>
            <input className="input" value={me?.email ?? ""} readOnly />
          </div>
          <div className="field">
            <label className="label">{t("profile.name")}</label>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <button className="btn btn-primary" disabled={busy !== "" || !name}>
            {t("profile.save")}
          </button>
        </form>
      )}

      {tab === "password" && (
        <form className="card content" onSubmit={savePassword}>
          <p className="muted">{t("profile.password_hint")}</p>
          <div className="field">
            <label className="label">{t("profile.current_password")}</label>
            <input
              className="input"
              type="password"
              value={currentPassword}
              onChange={(e) => setCurrentPassword(e.target.value)}
              autoComplete="current-password"
            />
          </div>
          <div className="field">
            <label className="label">{t("profile.new_password")}</label>
            <input
              className="input"
              type="password"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              autoComplete="new-password"
            />
          </div>
          <div className="field">
            <label className="label">{t("profile.confirm_password")}</label>
            <input
              className="input"
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              autoComplete="new-password"
            />
          </div>
          <button className="btn btn-primary" disabled={busy !== "" || !currentPassword || !newPassword}>
            {t("profile.change_password")}
          </button>
        </form>
      )}

      {tab === "email" && (
        <form className="card content" onSubmit={saveEmail}>
          <p className="muted">{t("profile.email_hint")}</p>
          <div className="field">
            <label className="label">{t("profile.username")}</label>
            <input className="input" value={me?.username ?? ""} readOnly />
          </div>
          <div className="field">
            <label className="label">{t("profile.new_email")}</label>
            <input
              className="input"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              autoComplete="email"
            />
          </div>
          <div className="field">
            <label className="label">{t("profile.current_password")}</label>
            <input
              className="input"
              type="password"
              value={emailPassword}
              onChange={(e) => setEmailPassword(e.target.value)}
              autoComplete="current-password"
            />
          </div>
          <button className="btn btn-primary" disabled={busy !== "" || !email || !emailPassword}>
            {t("profile.change_email")}
          </button>
        </form>
      )}

      {tab === "security" && (
        <div className="card content">
          <p>
            {t("security.status")}: <strong>{on ? t("security.on") : t("security.off")}</strong>
          </p>
          {!on && !secret && (
            <button className="btn btn-primary" disabled={busy !== ""} onClick={() => void start()}>
              {t("security.enable")}
            </button>
          )}
          {secret && (
            <form onSubmit={confirmTotp}>
              <p className="muted">{t("security.scan")}</p>
              {qr && (
                <Image
                  alt={t("security.qr")}
                  width={220}
                  height={220}
                  unoptimized
                  style={{ display: "block", maxWidth: "100%", height: "auto", background: "#fff", padding: 8 }}
                  src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(qr)}`}
                />
              )}
              <p>
                <label className="label">{t("security.secret")}</label>
                <code>{secret}</code>
              </p>
              {codes.length > 0 && (
                <div>
                  <p>{t("security.codes")}</p>
                  <ul>
                    {codes.map((c) => (
                      <li key={c}>
                        <code>{c}</code>
                      </li>
                    ))}
                  </ul>
                  <button className="btn" type="button" onClick={() => downloadCodes(codes)}>
                    {t("security.download")}
                  </button>
                </div>
              )}
              <div className="field">
                <label className="label">{t("login.otp")}</label>
                <input className="input" value={code} onChange={(e) => setCode(e.target.value)} inputMode="numeric" />
              </div>
              <button className="btn btn-primary" disabled={busy !== "" || !code}>
                {t("security.confirm")}
              </button>
            </form>
          )}
          {on && (
            <form onSubmit={disable}>
              <p className="muted">{t("security.disable_hint")}</p>
              <div className="field">
                <label className="label">{t("login.password")}</label>
                <input
                  className="input"
                  type="password"
                  value={disablePassword}
                  onChange={(e) => setDisablePassword(e.target.value)}
                />
              </div>
              <div className="field">
                <label className="label">{t("login.otp")}</label>
                <input className="input" value={code} onChange={(e) => setCode(e.target.value)} />
              </div>
              <button className="btn" disabled={busy !== "" || !disablePassword || !code}>
                {t("security.disable")}
              </button>
            </form>
          )}
        </div>
      )}
    </div>
  );
}

export default function ProfilePage() {
  return (
    <RequireAuth>
      <Suspense fallback={null}>
        <ProfileInner />
      </Suspense>
    </RequireAuth>
  );
}
