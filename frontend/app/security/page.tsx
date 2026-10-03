"use client";

// 2FA: включване / изключване през otpbaga (TOTP + резервни кодове).

import { FormEvent, useCallback, useEffect, useState } from "react";
import { RequireAuth } from "../../components/RequireAuth";
import { useI18n } from "../../components/I18nProvider";
import { api } from "../../lib/api";

interface Me {
  totp_enabled?: number;
  email?: string;
}

function SecurityInner() {
  const { t } = useI18n();
  const [me, setMe] = useState<Me | null>(null);
  const [secret, setSecret] = useState("");
  const [uri, setUri] = useState("");
  const [codes, setCodes] = useState<string[]>([]);
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [msg, setMsg] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");

  const load = useCallback(async () => {
    const r = await api.get<Me>("/v1/me");
    setMe(r);
  }, []);

  useEffect(() => {
    void load().catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, [load]);

  const start = async () => {
    setBusy("start");
    setError("");
    setMsg("");
    try {
      const r = await api.post<{ secret: string; uri: string; recovery_codes?: string[]; message?: string }>(
        "/v1/me/totp/start"
      );
      setSecret(r.secret);
      setUri(r.uri);
      setCodes(r.recovery_codes ?? []);
      setMsg(r.message || t("security.started"));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  };

  const confirm = async (e: FormEvent) => {
    e.preventDefault();
    setBusy("confirm");
    setError("");
    try {
      const r = await api.post<{ message?: string }>("/v1/me/totp/confirm", { code });
      setMsg(r.message || t("security.enabled"));
      setSecret("");
      setUri("");
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
      const r = await api.post<{ message?: string }>("/v1/me/totp/disable", { password, code });
      setMsg(r.message || t("security.disabled"));
      setPassword("");
      setCode("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  };

  const on = me?.totp_enabled === 1;

  return (
    <div>
      <div className="page-head">
        <h1 className="page-title">{t("security.title")}</h1>
      </div>
      {error && <div className="error-text">{error}</div>}
      {msg && <p className="muted">{msg}</p>}
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
          <form onSubmit={confirm}>
            <p className="muted">{t("security.scan")}</p>
            <p>
              <a href={uri}>{uri}</a>
            </p>
            <p>
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
              </div>
            )}
            <div className="field">
              <label className="label">{t("login.otp")}</label>
              <input className="input" value={code} onChange={(e) => setCode(e.target.value)} />
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
              <input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
            </div>
            <div className="field">
              <label className="label">{t("login.otp")}</label>
              <input className="input" value={code} onChange={(e) => setCode(e.target.value)} />
            </div>
            <button className="btn" disabled={busy !== "" || !password || !code}>
              {t("security.disable")}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}

export default function SecurityPage() {
  return (
    <RequireAuth>
      <SecurityInner />
    </RequireAuth>
  );
}
