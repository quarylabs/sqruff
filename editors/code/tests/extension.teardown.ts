import { test as teardown } from "@playwright/test";
import { rmSync } from "fs";
import { join } from "path";

teardown("remove .test_setup directory", () => {
  const testSetupDir = join(__dirname, "..", ".test_setup");
  rmSync(testSetupDir, { recursive: true, force: true });
  console.log("Cleaned up .test_setup directory.");
});
