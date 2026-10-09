"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

export default function Error({
  error,
  retry,
}: {
  error: Error & { digest?: string };
  retry: () => void;
}) {
  const router = useRouter();

  useEffect(() => {
    console.error("Route segment error:", error);
  }, [error]);

  return (
    <div className="content" style={{ display: "flex", justifyContent: "center" }}>
      <div className="card" style={{ maxWidth: 480, width: "100%", padding: 32, textAlign: "center" }}>
        <div style={{ fontSize: 48, marginBottom: 12 }}>⚠️</div>
        <h2 style={{ fontSize: 18, fontWeight: 700, margin: "0 0 8px" }}>
          Възникна грешка
        </h2>
        <p className="muted" style={{ margin: "0 0 20px" }}>
          Нещо се обърка при зареждането на страницата. Моля, опитайте отново.
        </p>
        {process.env.NODE_ENV === "development" && error.message && (
          <pre
            style={{
              background: "var(--danger-soft)",
              color: "var(--danger)",
              padding: 12,
              borderRadius: 8,
              fontSize: 12,
              textAlign: "left",
              overflow: "auto",
              marginBottom: 16,
            }}
          >
            {error.message}
          </pre>
        )}
        <div style={{ display: "flex", gap: 8, justifyContent: "center" }}>
          <button className="btn btn-primary" onClick={() => retry()}>
            Опитай отново
          </button>
          <button className="btn" onClick={() => router.push("/")}>
            Начало
          </button>
        </div>
      </div>
    </div>
  );
}
