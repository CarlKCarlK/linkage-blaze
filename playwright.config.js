import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/browser",
  use: {
    baseURL: process.env.CYD_TEST_BASE_URL ?? "http://127.0.0.1:8092",
    headless: true,
  },
});
