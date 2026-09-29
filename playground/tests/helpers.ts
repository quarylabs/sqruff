import { expect, Page } from "@playwright/test";

export async function updateEditorText(page: Page, text: string) {
  const editor = page.locator("#main .monaco-editor").filter({ visible: true });
  await editor.locator(".view-line").first().click();
  await page.keyboard.press("Control+A");
  await page.keyboard.insertText(text);
}

export async function formatEditorContains(page: Page, text: string) {
  await expect(page.locator("#secondary-panel")).toContainText(text);
}
