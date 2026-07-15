import { defineConfig } from "vitest/config";

// Frontend unit tests. Kept out of vite.config.ts so the app build config stays
// free of test-only concerns. jsdom (not a hand-rolled fake) because the island's
// event routing leans on real `closest()` traversal — the thing under test.
export default defineConfig({
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
});
