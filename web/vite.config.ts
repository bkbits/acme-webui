import { defineConfig } from "vite-plus";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:6980",
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: "dist",
  },
});
