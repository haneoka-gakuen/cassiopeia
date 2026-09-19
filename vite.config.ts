import { defineConfig } from "vite";
export default defineConfig({
  build: {
    lib: {
      entry: { index: "src/index.ts", plugin: "src/plugin.ts" },
      formats: ["es"],
      fileName: (_format, name) => name + ".js",
    },
    minify: false,
    sourcemap: true,
    target: "es2022",
  },
});
