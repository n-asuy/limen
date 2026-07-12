/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "1" enables the frontend performance recorder. Set by `bun run dev:perf`. */
  readonly VITE_PROFILING?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
