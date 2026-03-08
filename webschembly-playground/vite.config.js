import react from "@vitejs/plugin-react";
import path from "path";
import { defineConfig } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";

export default defineConfig({
  plugins: [
    react(),
    viteStaticCopy({
      targets: [
        {
          src: process.env.WEBSCHEMBLY_RUNTIME,
          dest: "wasm",
        },
      ],
    }),
  ],
  base: process.env.BASE_URL,
  resolve: {
    alias: [
      {
        find: "webschembly-js",
        replacement: path.resolve(__dirname, "../webschembly-js/src"),
      },
    ],
  },
});
