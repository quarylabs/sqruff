import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { chromium, defineConfig } from "@playwright/test";

function canLaunchChromium(): boolean {
  if (process.platform !== "linux") return true;

  const executable = chromium
    .executablePath()
    .replace(
      /\/chromium-(\d+)\/chrome-linux64\/chrome$/,
      "/chromium_headless_shell-$1/chrome-headless-shell-linux64/chrome-headless-shell",
    );
  if (!existsSync(executable)) return false;

  try {
    return !execFileSync("ldd", [executable], {
      encoding: "utf8",
    }).includes("not found");
  } catch {
    // Platforms without ldd may still provide a working browser runtime.
    return true;
  }
}

const chromiumAvailable = canLaunchChromium();
if (!chromiumAvailable) {
  console.warn("Skipping Playwright: Chromium host libraries are unavailable");
}

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
  projects: chromiumAvailable
    ? [
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
      ]
    : [],
});
