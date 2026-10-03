"use client";

// Вход — email/потребител + парола, опционално 2FA. JWT през jwtbaga.

import { FormEvent, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useAuth } from "../../components/AuthProvider";
import { useI18n } from "../../components/I18nProvider";
import { api, loginMfa } from "../../lib/api";

export default function LoginPage() {
  const { login } = useAuth();
  const { t } = useI18n();
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [mfaToken, setMfaToken] = useState("");
  const [regOn, setRegOn] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api
      .get<{ registration_enabled?: number }>("/v1/auth/registration-status")
      .then((r) => setRegOn(r.registration_enabled === 1))
      .catch(() => setRegOn(false));
  }, []);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      if (mfaToken) {
        await loginMfa(mfaToken, code);
        router.push("/");
        return;
      }
      const r = await login(username, password);
      if (r.mfa === "totp" && r.mfa_token) {
        setMfaToken(r.mfa_token);
        return;
      }
      router.push("/");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "";
      setError(msg || t("login.error"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="login-wrap">
      <form className="card login-card" onSubmit={handleSubmit}>
        <h1 className="login-title">{t("login.title")}</h1>
        {mfaToken ? (
          <div className="field">
            <label className="label">{t("login.otp")}</label>
            <input
              className="input"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              autoFocus
              autoComplete="one-time-code"
              inputMode="numeric"
            />
          </div>
        ) : (
          <>
            <div className="field">
              <label className="label">{t("login.username")}</label>
              <input
                className="input"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                autoFocus
                autoComplete="username"
              />
            </div>
            <div className="field">
              <label className="label">{t("login.password")}</label>
              <input
                className="input"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="current-password"
              />
            </div>
          </>
        )}
        {error && <div className="error-text">{error}</div>}
        <div className="form-actions">
          <button
            className="btn btn-primary"
            disabled={busy || (mfaToken ? !code : !username || !password)}
          >
            {busy ? t("login.submit") : mfaToken ? t("login.otp_submit") : t("login.submit")}
          </button>
        </div>
        {!mfaToken && (
          <p className="muted login-links">
            <Link href="/forgot-password">{t("login.forgot")}</Link>
            {regOn ? (
              <>
                {" · "}
                <Link href="/register">{t("login.register")}</Link>
              </>
            ) : null}
          </p>
        )}
      </form>
    </div>
  );
}
