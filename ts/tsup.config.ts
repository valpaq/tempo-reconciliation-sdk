import { defineConfig } from 'tsup'

export default defineConfig({
  entry: {
    index: 'src/index.ts',
    memo: 'src/memo/index.ts',
    watcher: 'src/watcher/index.ts',
    reconciler: 'src/reconciler/index.ts',
    export: 'src/export/index.ts',
    explorer: 'src/explorer/index.ts',
  },
  format: ['esm', 'cjs'],
  dts: true,
  sourcemap: true,
  clean: true,
  splitting: true,
  treeshake: true,
})
