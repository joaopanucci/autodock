import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import pkg from './package.json';

// The footer shows the version from package.json, so it can never go stale.
export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  // Monaco is loaded on demand; pre-bundling it at start keeps the dev server from discovering it
  // in the middle of a session and reloading the page.
  optimizeDeps: { include: ['monaco-editor'] },
});
