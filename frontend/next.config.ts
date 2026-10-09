import type { NextConfig } from "next";

const BACKEND = process.env.YTBUCH_API_PROXY ?? "http://127.0.0.1:8092";

const nextConfig: NextConfig = {
  output: "standalone",
  // Next блокира JS и HMR, ако адресът в браузъра не е в списъка.
  // 127.0.0.1 е този компютър. 192.168.1.145 е LAN адресът на машината.
  allowedDevOrigins: ["127.0.0.1", "localhost", "192.168.1.145", "192.168.1.146"],
  // Security headers — middleware.ts ги дублира с nonce, но тук са за
  // случаи, когато middleware не се изпълнява (статични файлове и др.).
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          {
            key: "X-Frame-Options",
            value: "DENY",
          },
          {
            key: "X-Content-Type-Options",
            value: "nosniff",
          },
          {
            key: "Referrer-Policy",
            value: "strict-origin-when-cross-origin",
          },
          {
            key: "Permissions-Policy",
            value: "camera=(), microphone=(), geolocation=()",
          },
        ],
      },
    ];
  },
  // Браузърът говори само с :3020. Next проксира API-то към ytbuch :8092.
  async rewrites() {
    return [
      { source: "/v1/:path*", destination: `${BACKEND}/v1/:path*` },
      { source: "/ready", destination: `${BACKEND}/ready` },
      { source: "/health", destination: `${BACKEND}/health` },
    ];
  },
};

export default nextConfig;
