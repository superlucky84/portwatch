import { defineConfig } from '@playwright/test';

const port = 4317;

export default defineConfig({
  testDir: 'tests/ui',
  fullyParallel: true,
  reporter: process.env.CI ? 'list' : 'line',
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    browserName: 'chromium',
    viewport: { width: 380, height: 520 },
    trace: 'off',
  },
  webServer: {
    command: `node scripts/serve-ui.mjs ${port}`,
    url: `http://127.0.0.1:${port}/index.html`,
    reuseExistingServer: !process.env.CI,
  },
});
