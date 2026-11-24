import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import { defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => {
  return {
    plugins: [react(), tailwindcss()],
    clearScreen: false,
    resolve: {
      alias: {
        '@': path.resolve(__dirname, '.'),
      },
    },
    define: {
      __APP_BUILD_DATE__: JSON.stringify(new Date().toISOString()),
    },
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: 'ws',
            host,
            port: 1421,
          }
        : process.env.DISABLE_HMR !== 'true',
      watch: {
        ignored: ['**/src-tauri/**'],
      },
    },
  };
});
