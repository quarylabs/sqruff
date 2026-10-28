import { cpSync } from "fs";
import { join } from "path";
import { test, expect } from "./fixtures";
import { FIXTURES_DIR, openServerPage, openFile, runCommand } from "./utils";

test("shows diagnostics for SQL lint errors", async ({
  sharedCodeServer,
  tempDir,
  page,
}) => {
  // Copy fixture project to temp dir
  const projectDir = join(tempDir, "sample_project");
  cpSync(FIXTURES_DIR, projectDir, { recursive: true, dereference: true });

  // Open code-server with the project
  await openServerPage(page, projectDir, sharedCodeServer);

  // Open the SQL file with lint errors
  await openFile(page, "lint_errors.sql");

  // Open the Problems panel so we can watch for diagnostics to appear
  await runCommand(page, "Problems: Focus on Problems View");

  // Wait for the extension to activate and produce diagnostics.
  // The file has "from" in lowercase which should trigger a lint error.
  // Use a single generous timeout instead of fixed delays.
  const problemsPanel = page.locator(".markers-panel-container");
  await expect(problemsPanel).toBeVisible({ timeout: 10_000 });

  const problemEntries = problemsPanel.locator(".monaco-list-row");
  await expect(problemEntries.first()).toBeVisible({ timeout: 30_000 });
});

test("formats SQL file via Format Document command", async ({
  sharedCodeServer,
  tempDir,
  page,
}) => {
  // Copy fixture project to temp dir
  const projectDir = join(tempDir, "sample_project");
  cpSync(FIXTURES_DIR, projectDir, { recursive: true, dereference: true });

  // Open code-server with the project
  await openServerPage(page, projectDir, sharedCodeServer);

  // Open the SQL file
  await openFile(page, "lint_errors.sql");

  // Diagnostics confirm that the language server is ready to format the file.
  await runCommand(page, "Problems: Focus on Problems View");
  await expect(
    page.locator(".markers-panel-container .monaco-list-row").first(),
  ).toBeVisible({ timeout: 30_000 });
  await runCommand(page, "View: Focus Active Editor Group");

  // Formatting is asynchronous. Wait for the changed SQL in the editor.
  await runCommand(page, "Format Document");
  const editor = page.locator(".monaco-editor").filter({ visible: true });
  await expect(editor.locator(".view-line").nth(3)).toHaveText(
    "FROM my_table",
    {
      timeout: 30_000,
    },
  );
});
