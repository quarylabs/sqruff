const esbuild = require("esbuild");

const production = process.argv.includes('--production');

const wasmPlugin = {
	name: "wasm",
	setup(build) {
		const path = require("path");
		const fs = require("fs");

		build.onResolve({ filter: /\.wasm$/ }, (args) => {
			return {
				path: path.isAbsolute(args.path)
					? args.path
					: path.join(args.resolveDir, args.path),
				namespace: "wasm-binary",
			};
		});

		build.onLoad(
			{ filter: /.*/, namespace: "wasm-binary" },
			async (args) => {
				return {
					contents: await fs.promises.readFile(args.path),
					loader: "binary",
				};
			},
		);
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
		minify: production
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
		external: [
			"vscode",
			"vscode-languageclient",
			"vscode-languageclient/node",
			"path",
			"fs",
		],
		outfile: "dist/native.js",
		platform: "node",
		format: "cjs",
		minify: production,
	})
	.catch(() => process.exit(1));
