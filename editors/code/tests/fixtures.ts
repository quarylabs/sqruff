import { test as base } from "@playwright/test";
import { mkdtempSync, rmSync } from "fs";
import { join } from "path";
import { tmpdir } from "os";
import {
  CodeServerContext,
  startCodeServer,
  stopCodeServer,
} from "./utils_code_server";

type WorkerFixtures = {
  sharedCodeServer: CodeServerContext;
};

type TestFixtures = {
  tempDir: string;
};

export const test = base.extend<TestFixtures, WorkerFixtures>({
  sharedCodeServer: [
    async ({}, use) => {
      const workerDir = mkdtempSync(join(tmpdir(), "sqruff-e2e-worker-"));
      const context = await startCodeServer({ tempDir: workerDir });
      await use(context);
      await stopCodeServer(context);
      rmSync(workerDir, { recursive: true, force: true });
    },
    { scope: "worker" },
  ],

  tempDir: async ({}, use) => {
    const dir = mkdtempSync(join(tmpdir(), "sqruff-e2e-test-"));
    await use(dir);
    rmSync(dir, { recursive: true, force: true });
  },
});

export { expect } from "@playwright/test";
