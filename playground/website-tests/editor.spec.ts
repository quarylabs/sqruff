import { test, expect } from "@playwright/test";

test("the playground editor formats SQL after an edit", async ({ page }) => {
  await page.goto("/?secondary=Format", { waitUntil: "domcontentloaded" });

  const editor = page.locator("#main .monaco-editor").filter({ visible: true });
  await expect(editor.locator(".view-lines")).toContainText(
    "SELECT name from USERS",
  );
  await expect(page.locator("#secondary-panel")).toContainText(
    "SELECT name FROM users",
  );

  await editor.locator(".view-line").first().click();
  await page.keyboard.press("Control+A");
  await page.keyboard.insertText("SELECT name from USERS_DEPLOYMENT_CHECK");

  await expect(editor.locator(".view-lines")).toHaveText(
    "SELECT name from USERS_DEPLOYMENT_CHECK",
  );
  await expect(page.locator("#secondary-panel")).toContainText(
    "SELECT name FROM users_deployment_check",
  );
});
