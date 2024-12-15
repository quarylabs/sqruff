import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests", // directory where your tests will be located
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  // Run tests in headless browsers by default
  use: {
    headless: true,
  },
  // Configure projects for different browsers if desired
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  // Automatically start Vite dev server
  webServer: {
    command: "pnpm run dev",
    url: "http://localhost:5173", // Vite default port
    reuseExistingServer: !process.env.CI,
  },
});
