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
  // Use Python's built-in http server to serve the built dist directory
  webServer: {
    command: "python3 -m http.server 4173 -d dist",
    url: "http://localhost:4173",
    reuseExistingServer: false,
  },
});
