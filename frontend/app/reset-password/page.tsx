"use client";

import { FormEvent, Suspense, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useI18n } from "../../components/I18nProvider";
import { api } from "../../lib/api";

function ResetInner() {
  const { t } = useI18n();
  const router = useRouter();
  const params = useSearchParams();
  const token = params.get("token") || "";
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    if (password.length < 8) {
      setError(t("register.need_password"));
      return;
    }
    if (password !== confirm) {
      setError(t("register.need_match"));
      return;
    }
    setBusy(true);
    try {
      await api.post("/v1/auth/reset-password", {
        token,
        password,
        password_confirmation: confirm,
      });
      router.push("/login");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="login-wrap">
      <form className="card login-card" onSubmit={submit}>
        <h1 className="login-title">{t("reset.title")}</h1>
        {!token && <p className="error-text">{t("reset.no_token")}</p>}
        <div className="field">
          <label className="label">{t("register.password")}</label>
          <input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
        </div>
        <div className="field">
          <label className="label">{t("register.confirm")}</label>
          <input className="input" type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
        </div>
        {error && <div className="error-text">{error}</div>}
        <div className="form-actions">
          <button className="btn btn-primary" disabled={busy || !token}>
            {t("reset.submit")}
          </button>
        </div>
        <p className="muted login-links">
          <Link href="/login">{t("login.title")}</Link>
        </p>
      </form>
    </div>
  );
}

export default function ResetPasswordPage() {
  return (
    <Suspense>
      <ResetInner />
    </Suspense>
  );
}
