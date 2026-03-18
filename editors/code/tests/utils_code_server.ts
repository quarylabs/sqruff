import { ChildProcess, spawn } from "child_process";
import { mkdirSync, writeFileSync } from "fs";
import { join, resolve } from "path";

export interface CodeServerContext {
  process: ChildProcess;
  port: number;
  url: string;
}

function randomPort(): number {
  return 50000 + Math.floor(Math.random() * 10000);
}

export async function startCodeServer({
  tempDir,
}: {
  tempDir: string;
}): Promise<CodeServerContext> {
  const port = randomPort();
  const extensionsDir = join(__dirname, "..", ".test_setup", "extensions");
  const userDataDir = join(tempDir, "user-data");

  // Pre-configure VS Code settings to disable workspace trust and welcome tab
  const settingsObj: Record<string, unknown> = {
    "security.workspace.trust.enabled": false,
    "workbench.startupEditor": "none",
  };
  if (process.env.SQRUFF_PATH) {
    settingsObj["sqruff.executablePath"] = resolve(process.env.SQRUFF_PATH);
  }
  const settings = JSON.stringify(settingsObj);
  for (const subdir of ["Machine", "User"]) {
    const settingsDir = join(userDataDir, subdir);
    mkdirSync(settingsDir, { recursive: true });
    writeFileSync(join(settingsDir, "settings.json"), settings);
  }

  const codeServerBin =
    process.env.CODE_SERVER_BIN ??
    join(__dirname, "..", "node_modules", ".bin", "code-server");

  const child = spawn(
    codeServerBin,
    [
      "--port",
      String(port),
      "--auth",
      "none",
      "--extensions-dir",
      extensionsDir,
      "--user-data-dir",
      userDataDir,
      "--disable-telemetry",
      "--disable-update-check",
    ],
    {
      cwd: join(__dirname, ".."),
      stdio: "pipe",
      env: {
        ...process.env,
        DISPLAY: "",
      },
    },
  );

  // Wait for code-server to report it's listening by monitoring stdout
  const url = `http://127.0.0.1:${port}`;
  await new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(new Error(`code-server did not start within 30s on port ${port}`));
    }, 30_000);

    const onData = (data: Buffer) => {
      const output = data.toString();
      process.stdout.write(`[code-server:${port}] ${output}`);
      if (output.includes("HTTP server listening on")) {
        clearTimeout(timeout);
        resolve();
      }
    };

    child.stdout?.on("data", onData);
    child.stderr?.on("data", (data: Buffer) => {
      process.stderr.write(`[code-server:${port}] ${data}`);
    });

    child.on("error", (err) => {
      clearTimeout(timeout);
      reject(err);
    });

    child.on("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`code-server exited with code ${code}`));
    });
  });

  return { process: child, port, url };
}

export async function stopCodeServer(
  context: CodeServerContext,
): Promise<void> {
  if (!context.process.killed) {
    context.process.kill("SIGTERM");
    await new Promise<void>((resolve) => {
      const timeout = setTimeout(() => {
        if (!context.process.killed) {
          context.process.kill("SIGKILL");
        }
        resolve();
      }, 5000);
      context.process.on("exit", () => {
        clearTimeout(timeout);
        resolve();
      });
    });
  }
}
