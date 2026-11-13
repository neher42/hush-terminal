import { defineConfig } from 'vitest/config';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    ...(host ? { hmr: { protocol: 'ws', host, port: 1421 } } : {}),
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: 'safari16',
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
    chunkSizeWarningLimit: 2000,
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
