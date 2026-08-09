import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { chromium, defineConfig, devices } from "@playwright/test";

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
  projects: chromiumAvailable
    ? [
        {
          name: "chromium",
          use: { ...devices["Desktop Chrome"] },
        },
      ]
    : [],
  // Vite and its Node runtime are declared Bazel inputs via VITE_DATA.
  webServer: {
    command:
      "node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 4173",
    url: "http://localhost:4173",
    reuseExistingServer: false,
  },
});
