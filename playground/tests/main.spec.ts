import { test, expect } from "@playwright/test";

test("home page opens", async ({ page }) => {
  // Navigate to the home page
  await page.goto("/");

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

test("state is saved when sql changes", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("link", { name: "quary Quary" })).toBeVisible();

  await page.getByLabel("Format").click();

  await page
    .locator("#main")
    .getByRole("code")
    .locator("div")
    .filter({ hasText: "SELECT name from USERS" })
    .nth(3)
    .click();

  await page.keyboard.press("Control+A")
  await page.keyboard.press("Backspace");
  await page.keyboard.type("SELECT name FROM users_test");

  await expect(page.locator("#secondary-panel")).toContainText(
    "SELECT name FROM users_test",
  );

  const url = new URL(page.url());

  expect(url.hash.length).toBeGreaterThan(10);
});

test("state is loaded", async ({ page }) => {
  await page.goto("/?secondary=Format#eNodijsKgDAMhq8SMru4Cp7EOpSaSqFE2j+dxLsbOn6Pl9EqbwypkoxWiiATGC+uzIre8Hqg9ZHzGfQqcY47RUUJ2kcVOKWnS1D+flhxHAs=");

  await expect(page.locator("#secondary-panel")).toContainText(
    "select 1 as test",
  );
});
