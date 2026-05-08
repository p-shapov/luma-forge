import antfu from "@antfu/eslint-config";

export default antfu({
  react: true,
  formatters: true,
  jsonc: true,
  markdown: true,
  yaml: true,
  toml: true,
  typescript: {
    filesTypeAware: ["src/**/*.{ts,tsx}"],
    tsconfigPath: "tsconfig.eslint.json",
  },
  stylistic: {
    indent: 2,
    quotes: "double",
    semi: true,
  },
  ignores: [
    "spec/**",
    "src-tauri/**",
    "src/generated/**",
  ],
}, {
  files: ["**/*.{js,jsx,ts,tsx,mjs,cjs}"],
  rules: {
    "style/max-len": ["error", {
      code: 120,
      ignoreRegExpLiterals: true,
      ignoreUrls: true,
      tabWidth: 2,
    }],
  },
}, {
  files: [
    "src/routes/**/*.tsx",
    "src/shared/components/ui/**/*.tsx",
  ],
  rules: {
    "react-refresh/only-export-components": "off",
  },
});
