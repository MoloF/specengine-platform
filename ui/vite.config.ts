// Vite and Vitest in one file (ui/README.md "Gates", "Laptop rules"): tests run once and exit,
// at most two workers, on jsdom, with a setup that fails a test writing to console.error or warn.
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  server: { host: "127.0.0.1" },
  preview: { host: "127.0.0.1" },
  test: {
    environment: "jsdom",
    watch: false,
    maxWorkers: 2,
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["./src/test/setup.ts"],
    // CSS reaches tests only as ?raw text (the token and focus checks); without this it is empty.
    css: true,
  },
});
