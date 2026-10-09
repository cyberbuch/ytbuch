"use client";

import { useEffect } from "react";

export default function GlobalError({
  error,
  retry,
}: {
  error: Error & { digest?: string };
  retry: () => void;
}) {
  useEffect(() => {
    console.error("Global error:", error);
  }, [error]);

  return (
    <html lang="bg">
      <body
        style={{
          margin: 0,
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "#f2f8f5",
          color: "#17211c",
          fontFamily:
            'ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif',
          fontSize: 14,
          lineHeight: 1.5,
        }}
      >
        <div
          style={{
            maxWidth: 480,
            width: "calc(100% - 32px)",
            padding: 32,
            textAlign: "center",
            background: "#ffffff",
            border: "1px solid #d9e8e0",
            borderRadius: 12,
            boxShadow: "0 10px 24px rgba(16, 24, 40, 0.14)",
          }}
        >
          <div style={{ fontSize: 48, marginBottom: 12 }}>💥</div>
          <h2 style={{ fontSize: 18, fontWeight: 700, margin: "0 0 8px" }}>
            Критична грешка
          </h2>
          <p style={{ color: "#64756c", margin: "0 0 20px" }}>
            Приложението срещна неочаквана грешка. Моля, презаредете страницата.
          </p>
          {process.env.NODE_ENV === "development" && error.message && (
            <pre
              style={{
                background: "#fdecec",
                color: "#dc2626",
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
            <button
              onClick={() => retry()}
              style={{
                padding: "8px 14px",
                borderRadius: 8,
                border: "1px solid #0e8c6a",
                background: "#0e8c6a",
                color: "#fff",
                fontWeight: 500,
                cursor: "pointer",
              }}
            >
              Опитай отново
            </button>
            <button
              onClick={() => window.location.reload()}
              style={{
                padding: "8px 14px",
                borderRadius: 8,
                border: "1px solid #bcd4c8",
                background: "#ffffff",
                color: "#17211c",
                fontWeight: 500,
                cursor: "pointer",
              }}
            >
              Презареди
            </button>
          </div>
        </div>
      </body>
    </html>
  );
}
