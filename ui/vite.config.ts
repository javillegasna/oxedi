import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

export default defineConfig({
  resolve: {
    alias: {
      'oxedi-engine': fileURLToPath(new URL('../viewer/pkg/index.js', import.meta.url)),
    },
  },
  build: {
    lib: {
      entry: 'src/index.ts',
      formats: ['es'],
      fileName: () => 'oxedi-viewer.js',
    },
    rollupOptions: {
      output: { codeSplitting: false },
    },
  },
});
