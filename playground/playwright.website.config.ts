import { defineConfig, devices } from "@playwright/test";

// Bazel stages the combined deployment artifact at ../website in runfiles.
export default defineConfig({
  testDir: "./website-tests",
  use: {
    baseURL: "http://127.0.0.1:4174",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command:
      "node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 4174 --outDir ../website",
    url: "http://127.0.0.1:4174/docs/",
    reuseExistingServer: false,
  },
});
