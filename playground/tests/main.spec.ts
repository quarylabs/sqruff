import { test, expect, type Locator } from "@playwright/test";
import { formatEditorContains, updateEditorText } from "./helpers";

test("home page opens", async ({ page }) => {
  // Navigate to the home page
  await page.goto("/");

  // Check if the main heading or any expected element is visible
  // For a React app created by Vite + React template, the initial content often includes <h1>Vite + React</h1>
  await expect(page.getByRole("link", { name: "quary Quary" })).toBeVisible();

  // Click on format and check if the format page opens and shows sql
  await page.getByLabel("Format").click();
  await expect(page.locator("#main")).toContainText("SELECT name from USERS");

  await formatEditorContains(page, "SELECT name FROM users");
});

test("state is saved when sql changes", async ({ page }) => {
  await page.goto("/");

  await page.getByLabel("Format").click();

  await updateEditorText(page, "SELECT name FROM users_test");

  await formatEditorContains(page, "SELECT name FROM users_test");

  const url = new URL(page.url());

  expect(url.hash.length).toBeGreaterThan(10);
});

test("state is loaded", async ({ page }) => {
  await page.goto(
    "/?secondary=Format#eNodijsKgDAMhq8SMru4Cp7EOpSaSqFE2j+dxLsbOn6Pl9EqbwypkoxWiiATGC+uzIre8Hqg9ZHzGfQqcY47RUUJ2kcVOKWnS1D+flhxHAs=",
  );

  await formatEditorContains(page, "select 1 as test");
});

test("source editor provides semantic tokens", async ({ page }) => {
  await page.goto("/?__sqruffSemanticTokenTest=1");

  const tokenData = await page
    .waitForFunction(() => {
      const data = (
        window as typeof window & {
          __sqruffLastSemanticTokens?: number[];
        }
      ).__sqruffLastSemanticTokens;
      return data && data.length >= 10 ? data : undefined;
    })
    .then((handle) => handle.jsonValue());

  expect(tokenData.slice(0, 5)).toEqual([0, 0, 6, 0, 0]);
  expect(chunkSemanticTokens(tokenData).some((token) => token[3] === 7)).toBe(
    true,
  );

  const sourceEditor = page.locator("#main .monaco-editor").filter({
    visible: true,
  });
  await expect(sourceEditor.locator(".view-line").first()).toContainText(
    "SELECT",
  );

  await expect
    .poll(() => renderedTokenColors(sourceEditor.locator(".view-lines")))
    .toMatchObject({
      SELECT: "rgb(255, 0, 0)",
      name: "rgb(0, 255, 0)",
    });
});

async function renderedTokenColors(viewLines: Locator) {
  return viewLines.locator("span").evaluateAll((spans) =>
    Object.fromEntries(
      spans
        .filter((span) => span.children.length === 0)
        .map((span) => [
          span.textContent?.trim() ?? "",
          getComputedStyle(span).color,
        ])
        .filter(([text]) => text.length > 0),
    ),
  );
}

function chunkSemanticTokens(data: number[]): number[][] {
  const chunks: number[][] = [];
  for (let index = 0; index < data.length; index += 5) {
    chunks.push(data.slice(index, index + 5));
  }
  return chunks;
}
