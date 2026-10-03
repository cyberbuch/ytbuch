"use client";

// Регистрация с нова фирма. Спряна ако auth.registration_enabled=0.

import { FormEvent, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useI18n } from "../../components/I18nProvider";
import { api } from "../../lib/api";

export default function RegisterPage() {
  const { t } = useI18n();
  const router = useRouter();
  const [step, setStep] = useState(1);
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [form, setForm] = useState({
    name: "",
    email: "",
    password: "",
    password_confirmation: "",
    company_name: "",
    company_eik: "",
    company_vat_number: "",
    company_address: "",
    company_city: "",
  });

  useEffect(() => {
    void api
      .get<{ registration_enabled?: number }>("/v1/auth/registration-status")
      .then((r) => setEnabled(r.registration_enabled === 1))
      .catch(() => setEnabled(false));
  }, []);

  const set = (k: string, v: string) => setForm((p) => ({ ...p, [k]: v }));

  const next = (e: FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.name.trim() || !form.email.includes("@")) {
      setError(t("register.need_user"));
      return;
    }
    if (form.password.length < 8) {
      setError(t("register.need_password"));
      return;
    }
    if (form.password !== form.password_confirmation) {
      setError(t("register.need_match"));
      return;
    }
    setStep(2);
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.company_name.trim() || !form.company_eik.trim()) {
      setError(t("register.need_company"));
      return;
    }
    setBusy(true);
    try {
      await api.post("/v1/auth/register", form);
      router.push("/login");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  if (enabled === null) {
    return <div className="login-wrap muted">{t("common.loading")}</div>;
  }
  if (!enabled) {
    return (
      <div className="login-wrap">
        <div className="card login-card">
          <h1 className="login-title">{t("register.title")}</h1>
          <p className="error-text">{t("register.disabled")}</p>
          <p className="muted login-links">
            <Link href="/login">{t("login.title")}</Link>
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="login-wrap">
      <form className="card login-card login-card-wide" onSubmit={step === 1 ? next : submit}>
        <h1 className="login-title">{t("register.title")}</h1>
        <p className="muted" style={{ marginTop: 0 }}>
          {step === 1 ? t("register.step_user") : t("register.step_company")}
        </p>
        {step === 1 ? (
          <>
            <div className="field">
              <label className="label">{t("register.name")}</label>
              <input className="input" value={form.name} onChange={(e) => set("name", e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("register.email")}</label>
              <input className="input" type="email" value={form.email} onChange={(e) => set("email", e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("register.password")}</label>
              <input className="input" type="password" value={form.password} onChange={(e) => set("password", e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("register.confirm")}</label>
              <input className="input" type="password" value={form.password_confirmation} onChange={(e) => set("password_confirmation", e.target.value)} />
            </div>
          </>
        ) : (
          <>
            <div className="field">
              <label className="label">{t("register.company_name")}</label>
              <input className="input" value={form.company_name} onChange={(e) => set("company_name", e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("register.company_eik")}</label>
              <input className="input" value={form.company_eik} onChange={(e) => set("company_eik", e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("register.company_vat")}</label>
              <input className="input" value={form.company_vat_number} onChange={(e) => set("company_vat_number", e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("register.company_city")}</label>
              <input className="input" value={form.company_city} onChange={(e) => set("company_city", e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("register.company_address")}</label>
              <input className="input" value={form.company_address} onChange={(e) => set("company_address", e.target.value)} />
            </div>
          </>
        )}
        {error && <div className="error-text">{error}</div>}
        <div className="form-actions">
          {step === 2 && (
            <button type="button" className="btn" onClick={() => setStep(1)}>
              {t("register.back")}
            </button>
          )}
          <button className="btn btn-primary" disabled={busy}>
            {step === 1 ? t("register.next") : t("register.submit")}
          </button>
        </div>
        <p className="muted login-links">
          <Link href="/login">{t("login.title")}</Link>
        </p>
      </form>
    </div>
  );
}
