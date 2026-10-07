import { defineConfig, devices } from "@playwright/test";

const deployedURL = process.env.WEBSITE_BASE_URL;

// Use the combined artifact locally, or the deployed site for the smoke test.
export default defineConfig({
  testDir: "./website-tests",
  retries: deployedURL ? 2 : 0,
  use: {
    baseURL: deployedURL ?? "http://127.0.0.1:4174",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: deployedURL
    ? undefined
    : {
        command:
          "node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 4174 --outDir ../website",
        url: "http://127.0.0.1:4174/docs/",
        reuseExistingServer: false,
      },
});
