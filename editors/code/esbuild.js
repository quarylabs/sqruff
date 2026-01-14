const esbuild = require("esbuild");
const path = require("path");
const fs = require("fs");

const production = process.argv.includes("--production");

// Support Bazel builds by allowing WASM location override via env var
const wasmDir = process.env.SQRUFF_WASM_DIR || "dist";

const wasmPlugin = {
  name: "wasm",
  setup(build) {
    // Redirect ../dist/sqruff_lsp imports to the actual WASM location
    build.onResolve({ filter: /\.\.\/dist\/sqruff_lsp/ }, (args) => {
      let filename = path.basename(args.path);
      // Handle .wasm files specially
      if (filename.endsWith(".wasm")) {
        const resolved = path.join(__dirname, wasmDir, filename);
        return { path: resolved, namespace: "wasm-binary" };
      }
      // Add .js extension if not already present (TypeScript imports omit it)
      if (!filename.endsWith(".js")) {
        filename = filename + ".js";
      }
      const resolved = path.join(__dirname, wasmDir, filename);
      return { path: resolved };
    });

    build.onResolve({ filter: /\.wasm$/ }, (args) => {
      return {
        path: path.isAbsolute(args.path)
          ? args.path
          : path.join(args.resolveDir, args.path),
        namespace: "wasm-binary",
      };
    });

    build.onLoad({ filter: /.*/, namespace: "wasm-binary" }, async (args) => {
      return {
        contents: await fs.promises.readFile(args.path),
        loader: "binary",
      };
    });
  },
};

esbuild
  .build({
    entryPoints: ["src/browser.ts"],
    bundle: true,
    external: ["vscode"],
    outfile: "dist/browser.js",
    format: "cjs",
    platform: "browser",
    minify: production,
  })
  .catch(() => process.exit(1));

esbuild
  .build({
    entryPoints: ["src/lsp-worker.ts"],
    bundle: true,
    outfile: "dist/browserServerMain.js",
    format: "iife",
    platform: "browser",
    plugins: [wasmPlugin],
    minify: production,
  })
  .catch(() => process.exit(1));

esbuild
  .build({
    entryPoints: ["src/native.ts"],
    bundle: true,
    external: ["vscode", "path", "fs"],
    outfile: "dist/native.cjs",
    platform: "node",
    format: "cjs",
    minify: production,
  })
  .catch(() => process.exit(1));
