import { defineConfig, devices } from "@playwright/test";

const deployedURL = process.env.WEBSITE_BASE_URL;

// Use the combined artifact locally, or the deployed site for the smoke test.
export default defineConfig({
  testDir: "./website-tests",
  outputDir: process.env.TEST_UNDECLARED_OUTPUTS_DIR ?? "test-results",
  snapshotPathTemplate: `${process.env.WEBSITE_SNAPSHOT_DIR ?? "{testDir}/snapshots"}/{platform}/{arg}{ext}`,
  updateSnapshots: "none",
  expect: {
    toHaveScreenshot: {
      animations: "disabled",
      caret: "hide",
      maxDiffPixels: 100,
    },
  },
  retries: deployedURL ? 2 : 0,
  use: {
    baseURL: deployedURL ?? "http://127.0.0.1:4174",
    trace: "retain-on-failure",
    colorScheme: "light",
    reducedMotion: "reduce",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1280, height: 720 },
      },
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
