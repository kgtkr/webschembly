import path from "path";
import { defineConfig } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";

export default defineConfig({
  plugins: [
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
