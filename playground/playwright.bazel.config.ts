import { defineConfig, devices } from "@playwright/test";

// Bazel-specific Playwright config that serves the built dist directory
export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  use: {
    headless: true,
    baseURL: "http://localhost:4173",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  // Vite and its Node runtime are declared Bazel inputs via VITE_DATA.
  webServer: {
    command:
      "node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 4173",
    url: "http://localhost:4173",
    reuseExistingServer: false,
  },
});
