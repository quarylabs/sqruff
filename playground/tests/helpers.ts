import { expect, Page } from "@playwright/test";

export async function updateEditorText(page: Page, text: string) {
  // Focus the editor first
  await page
    .locator("#main")
    .getByRole("code")
    .locator("div .view-line")
    .nth(0)
    .click();

  // Use Monaco Editor's API directly to set the text, which is more reliable than keyboard typing
  await page.evaluate((newText) => {
    // Get all Monaco Editor instances
    const editors = (
      window as unknown as {
        monaco?: {
          editor?: {
            getEditors?: () => Array<{ setValue: (text: string) => void }>;
          };
        };
      }
    ).monaco?.editor?.getEditors?.();
    if (editors && editors.length > 0) {
      // Find the first visible/active editor in the main panel
      // The visible editor should be the one we want to update
      for (const editor of editors) {
        const container = (
          editor as unknown as { getContainerDomNode: () => HTMLElement }
        ).getContainerDomNode();
        if (
          container &&
          container.closest("#main") &&
          container.offsetParent !== null
        ) {
          editor.setValue(newText);
          return;
        }
      }
      // Fallback: set the first editor
      editors[0].setValue(newText);
    }
  }, text);

  // Small wait for React to process the change
  await page.waitForTimeout(100);
}

export async function formatEditorContains(page: Page, text: string) {
  await expect(page.locator("#secondary-panel")).toContainText(text);
}
