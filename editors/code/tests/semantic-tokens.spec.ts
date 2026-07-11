import { test as browserTest, expect, type Locator } from "@playwright/test";
import { cpSync, readFileSync } from "fs";
import { join } from "path";
import { test } from "./fixtures";
import { FIXTURES_DIR, openFile, openServerPage } from "./utils";

const WORKER_ORIGIN = "http://sqruff-worker.test";

browserTest("browser LSP worker returns semantic tokens", async ({ page }) => {
  const workerScript = readFileSync(
    join(__dirname, "..", "dist", "browserServerMain.js"),
    "utf-8",
  );

  await page.route(`${WORKER_ORIGIN}/`, async (route) => {
    await route.fulfill({
      contentType: "text/html",
      body: "<!doctype html><title>sqruff worker test</title>",
    });
  });

  await page.route(`${WORKER_ORIGIN}/browserServerMain.js`, async (route) => {
    await route.fulfill({
      contentType: "application/javascript",
      body: workerScript,
    });
  });

  await page.goto(WORKER_ORIGIN);

  const tokenData = await page.evaluate(async () => {
    const worker = new Worker("/browserServerMain.js");
    const pending = new Map<
      number,
      { resolve: (value: unknown) => void; reject: (error: Error) => void }
    >();
    let nextId = 1;
    let resolveReady!: () => void;
    let resolveConfigLoaded!: () => void;

    const ready = new Promise<void>((resolve) => {
      resolveReady = resolve;
    });
    const configLoaded = new Promise<void>((resolve) => {
      resolveConfigLoaded = resolve;
    });

    worker.onmessage = ({ data }) => {
      if (data === "OK") {
        resolveReady();
        return;
      }

      if (data.method === "loadConfig") {
        worker.postMessage({
          jsonrpc: "2.0",
          id: data.id,
          result: "[sqruff]\ndialect = ansi\n",
        });
        resolveConfigLoaded();
        return;
      }

      if (typeof data.id === "number" && pending.has(data.id)) {
        const request = pending.get(data.id)!;
        pending.delete(data.id);
        if (data.error) {
          request.reject(new Error(data.error.message));
        } else {
          request.resolve(data.result);
        }
      }
    };

    function sendRequest(method: string, params: unknown): Promise<unknown> {
      const id = nextId++;
      worker.postMessage({ jsonrpc: "2.0", id, method, params });
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
      });
    }

    function sendNotification(method: string, params: unknown): void {
      worker.postMessage({ jsonrpc: "2.0", method, params });
    }

    await ready;
    await sendRequest("initialize", {
      processId: null,
      rootUri: "file:///workspace",
      capabilities: {
        textDocument: {
          semanticTokens: {
            requests: { full: true },
            tokenTypes: [
              "keyword",
              "string",
              "number",
              "comment",
              "operator",
              "function",
              "type",
              "variable",
              "parameter",
              "property",
              "macro",
            ],
            tokenModifiers: [],
            formats: ["relative"],
          },
        },
      },
    });
    sendNotification("initialized", {});
    await configLoaded;

    const uri = "file:///workspace/query.sql";
    sendNotification("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "sql",
        version: 1,
        text: "SELECT name FROM users WHERE age > 1",
      },
    });

    const semanticTokens = (await sendRequest(
      "textDocument/semanticTokens/full",
      {
        textDocument: { uri },
      },
    )) as { data: number[] };

    worker.terminate();
    return semanticTokens.data;
  });

  expect(tokenData.slice(0, 5)).toEqual([0, 0, 6, 0, 0]);
  expect(chunkSemanticTokens(tokenData).some((token) => token[3] === 7)).toBe(
    true,
  );
});

test("VS Code renders distinct semantic token colours", async ({
  sharedCodeServer,
  tempDir,
  page,
}) => {
  const projectDir = join(tempDir, "sample_project");
  cpSync(FIXTURES_DIR, projectDir, { recursive: true });

  await openServerPage(page, projectDir, sharedCodeServer);
  await openFile(page, "lint_errors.sql");

  const editor = page.locator(".monaco-editor").filter({ visible: true });
  await expect(editor.locator(".view-line").first()).toContainText("SELECT");

  await expect
    .poll(() => renderedTokenColors(editor.locator(".view-lines")), {
      timeout: 30_000,
    })
    .toMatchObject({
      SELECT: "rgb(255, 0, 0)",
      a: "rgb(0, 255, 0)",
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
