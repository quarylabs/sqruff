import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: true,
  retries: 2,
  workers: 1,
  reporter: "html",
  timeout: 60_000,
  use: {
    viewport: { width: 1512, height: 944 },
    video: "retain-on-failure",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "setup",
      testMatch: /extension\.setup\.ts/,
    },
    {
      name: "electron-vscode",
      testMatch: /\.spec\.ts$/,
      dependencies: ["setup"],
      use: {
        browserName: "chromium",
        headless: true,
      },
    },
    {
      name: "cleanup",
      testMatch: /extension\.teardown\.ts/,
      dependencies: ["electron-vscode"],
    },
  ],
});
