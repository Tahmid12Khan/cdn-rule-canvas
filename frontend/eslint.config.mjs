// ESLint 10 flat config. Next 16's `next lint` is removed, and ESLint 10 no
// longer reads `.eslintrc.*`, so we consume eslint-config-next's flat configs
// directly (their CJS module.exports is the flat-config array) and run the
// ESLint CLI from package.json. Mirrors the previous extends:
// ["next/core-web-vitals", "next/typescript"] plus the no-explicit-any rule.
import coreWebVitals from "eslint-config-next/core-web-vitals";
import typescript from "eslint-config-next/typescript";

export default [
  ...coreWebVitals,
  ...typescript,
  {
    rules: {
      "@typescript-eslint/no-explicit-any": "error",
      // React-Compiler readiness rules newly shipped by eslint-plugin-react-hooks
      // v6 (pulled in by eslint-config-next 16). They flag long-standing,
      // intentional patterns (controlled-input prop→state sync, ref access) that
      // predate the rules. Turned off so the dependency upgrade doesn't require
      // rewriting working components; revisit as a dedicated React-Compiler pass.
      "react-hooks/set-state-in-effect": "off",
      "react-hooks/refs": "off",
      "react-hooks/immutability": "off",
      "react-hooks/error-boundaries": "off",
    },
  },
  {
    ignores: ["e2e/**", "playwright.config.ts", ".next/**"],
  },
];
