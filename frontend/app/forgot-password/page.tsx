"use client";

import { FormEvent, useState } from "react";
import Link from "next/link";
import { useI18n } from "../../components/I18nProvider";
import { api } from "../../lib/api";

export default function ForgotPasswordPage() {
  const { t } = useI18n();
  const [email, setEmail] = useState("");
  const [msg, setMsg] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError("");
    setMsg("");
    try {
      const r = await api.post<{ message?: string }>("/v1/auth/forgot-password", { email });
      setMsg(r.message || t("forgot.ok"));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="login-wrap">
      <form className="card login-card" onSubmit={submit}>
        <h1 className="login-title">{t("forgot.title")}</h1>
        <p className="muted" style={{ marginTop: 0 }}>{t("forgot.hint")}</p>
        <div className="field">
          <label className="label">{t("register.email")}</label>
          <input className="input" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
        </div>
        {error && <div className="error-text">{error}</div>}
        {msg && <div className="muted">{msg}</div>}
        <div className="form-actions">
          <button className="btn btn-primary" disabled={busy || !email}>
            {t("forgot.submit")}
          </button>
        </div>
        <p className="muted login-links">
          <Link href="/login">{t("login.title")}</Link>
        </p>
      </form>
    </div>
  );
}
