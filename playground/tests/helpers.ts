import { expect, Page } from "@playwright/test";

export async function updateEditorText(page: Page, text: string) {
  await page
    .locator("#main")
    .getByRole("code")
    .locator("div .view-line")
    .nth(0)
    .click();

  await page.keyboard.press("Control+A");
  await page.keyboard.press("Backspace");
  await page.keyboard.type(text);
}

export async function formatEditorContains(page: Page, text: string) {
  await expect(page.locator("#secondary-panel")).toContainText(text);
}
