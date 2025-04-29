import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  build: {
    target: 'esnext',
    commonjsOptions: {
      include: [/node_modules/],
      transformMixedEsModules: true,
    },
    rollupOptions: {
      external: ['@demox-labs/miden-sdk'],
      output: {
        format: "es",
      },
    },
  },
  optimizeDeps: {
    include: ['@demox-labs/miden-sdk'],
    esbuildOptions: {
      target: "esnext",
      supported: {
        "top-level-await": true, 
      },
    },
  },
});