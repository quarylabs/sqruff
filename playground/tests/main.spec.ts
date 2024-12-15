import { test, expect } from "@playwright/test";

test("home page opens", async ({ page }) => {
  // Navigate to the home page
  await page.goto("http://localhost:5173");

  // Check if the main heading or any expected element is visible
  // For a React app created by Vite + React template, the initial content often includes <h1>Vite + React</h1>
  await page.getByRole("link", { name: "quary Quary" }).isVisible();
});
