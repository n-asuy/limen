import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import fs from "node:fs";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// Profiling is opt-in and mirrors the backend's `profiling` cargo feature:
// both are enabled together by `bun run dev:perf`. A plain `bun run dev`
// writes nothing to `log/`.
// @ts-expect-error process is a nodejs global
const profiling = process.env.VITE_PROFILING === "1";

// ---- Perf log middleware ----

function perfLogDir(): string {
  return path.resolve(__dirname, "log", "performance");
}

function dateString(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function perfReportPlugin(): Plugin {
  return {
    name: "perf-report",
    configureServer(server) {
      server.middlewares.use("/perf/report", (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405;
          res.end();
          return;
        }
        const chunks: Buffer[] = [];
        req.on("data", (chunk: Buffer) => chunks.push(chunk));
        req.on("end", () => {
          try {
            const body = Buffer.concat(chunks).toString("utf-8");
            const entries: unknown[] = JSON.parse(body);
            const dir = perfLogDir();
            fs.mkdirSync(dir, { recursive: true });
            const filePath = path.join(dir, `${dateString()}_frontend.jsonl`);
            const lines = entries.map((e) => JSON.stringify(e)).join("\n") + "\n";
            fs.appendFileSync(filePath, lines, "utf-8");
            res.statusCode = 200;
            res.end("ok");
          } catch {
            res.statusCode = 400;
            res.end("bad request");
          }
        });
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), ...(profiling ? [perfReportPlugin()] : [])],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. Use a random available port to avoid conflicts
  server: {
    port: 5199,
    strictPort: false,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri` and perf logs
      ignored: ["**/src-tauri/**", "**/log/**"],
    },
  },
}));
