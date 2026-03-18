import { Page } from "@playwright/test";
import { join } from "path";
import { CodeServerContext } from "./utils_code_server";

export const EXTENSION_DIR = join(__dirname, "..");
export const FIXTURES_DIR = join(__dirname, "fixtures", "sample_project");

export async function openServerPage(
  page: Page,
  targetPath: string,
  context: CodeServerContext,
): Promise<void> {
  await page.goto(
    `http://127.0.0.1:${context.port}/?folder=${encodeURIComponent(targetPath)}`,
  );
  await page.waitForLoadState("networkidle");
  await page.waitForSelector('[role="application"]', { timeout: 30_000 });

  // Dismiss the "Do you trust the authors?" dialog if it appears
  const trustButton = page.locator(
    'button:has-text("Yes, I trust the authors")',
  );
  try {
    await trustButton.click({ timeout: 5000 });
  } catch {
    // Dialog may not appear if workspace trust is disabled
  }

  // Give extensions time to activate
  await page.waitForTimeout(3000);
}

export async function runCommand(page: Page, command: string): Promise<void> {
  const maxRetries = 3;
  const retryDelay = 3000;

  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      await page.keyboard.press(
        process.platform === "darwin" ? "Meta+Shift+P" : "Control+Shift+P",
      );
      await page.waitForSelector(
        'input[aria-label="Type the name of a command to run."]',
        { timeout: 5000 },
      );
      await page.keyboard.type(command);
      const commandElement = await page.waitForSelector(
        `a:has-text("${command}")`,
        { timeout: 5000 },
      );
      await commandElement!.click();
      return;
    } catch (error) {
      if (attempt === maxRetries - 1) {
        throw error;
      }
      await page.keyboard.press("Escape");
      await page.waitForTimeout(retryDelay);
    }
  }
}

export async function openFile(page: Page, filename: string): Promise<void> {
  const maxRetries = 3;
  const retryDelay = 3000;

  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      await page.keyboard.press(
        process.platform === "darwin" ? "Meta+P" : "Control+P",
      );
      await page
        .getByRole("textbox", { name: "Search files by name" })
        .waitFor({ state: "visible", timeout: 5000 });
      await page.keyboard.type(filename);
      const fileElement = await page.waitForSelector(
        `a:has-text("${filename}")`,
        { timeout: 5000 },
      );
      await fileElement!.click();
      return;
    } catch (error) {
      if (attempt === maxRetries - 1) {
        throw error;
      }
      await page.keyboard.press("Escape");
      await page.waitForTimeout(retryDelay);
    }
  }
}
