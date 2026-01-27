// @ts-check

import tseslint from "typescript-eslint";

export default tseslint.config(
  tseslint.configs.eslintRecommended,
  tseslint.configs.recommended,
  {
    ignores: ["src/pkg/**/*.{ts,js}"],
  },
);
