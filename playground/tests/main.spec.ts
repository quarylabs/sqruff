import { test, expect } from "@playwright/test";

test("home page opens", async ({ page }) => {
  // Navigate to the home page
  await page.goto("http://localhost:5173");

  // Check if the main heading or any expected element is visible
  // For a React app created by Vite + React template, the initial content often includes <h1>Vite + React</h1>
  await expect(page.getByRole("link", { name: "quary Quary" })).toBeVisible();

  // Click on format and check if the format page opens and shows sql
  await page.getByLabel("Format").click();
  await expect(page.locator("#main")).toContainText("SELECT name from USERS");
  await expect(page.locator("#secondary-panel")).toContainText(
    "SELECT name FROM users",
  );
});
