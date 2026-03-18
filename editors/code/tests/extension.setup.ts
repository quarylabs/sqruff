import { test as setup } from "@playwright/test";
import { execSync } from "child_process";
import { createHash } from "crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { join } from "path";

const CODE_SERVER_BIN =
  process.env.CODE_SERVER_BIN ??
  join(__dirname, "..", "node_modules", ".bin", "code-server");
const EXTENSIONS_DIR = join(__dirname, "..", ".test_setup", "extensions");
const HASH_FILE = join(__dirname, "..", ".test_setup", "vsix.sha256");

setup("install extension into code-server", () => {
  const vsixPath = process.env.VSIX_PATH
    ? join(__dirname, "..", process.env.VSIX_PATH)
    : (() => {
        const packageJson = JSON.parse(
          readFileSync(join(__dirname, "..", "package.json"), "utf-8"),
        );
        return join(__dirname, "..", `sqruff-${packageJson.version}.vsix`);
      })();

  if (!existsSync(vsixPath)) {
    throw new Error(
      `VSIX not found at ${vsixPath}. Run "pnpm run package" first.`,
    );
  }

  const vsixHash = createHash("sha256")
    .update(readFileSync(vsixPath))
    .digest("hex");

  if (existsSync(HASH_FILE)) {
    const cachedHash = readFileSync(HASH_FILE, "utf-8").trim();
    if (cachedHash === vsixHash) {
      console.log("VSIX unchanged, skipping install.");
      return;
    }
  }

  mkdirSync(EXTENSIONS_DIR, { recursive: true });

  console.log(`Installing ${vsixPath} into ${EXTENSIONS_DIR}...`);
  execSync(
    `${CODE_SERVER_BIN} --install-extension ${vsixPath} --extensions-dir ${EXTENSIONS_DIR}`,
    {
      cwd: join(__dirname, ".."),
      stdio: "inherit",
    },
  );

  writeFileSync(HASH_FILE, vsixHash);
});
