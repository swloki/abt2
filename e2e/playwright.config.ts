import { defineConfig, devices } from '@playwright/test';
import 'dotenv/config';

/**
 * ABT E2E 配置。
 * - globalSetup 走 API 登录 /login → storageState 到 .auth/admin.json
 * - workers:1 串行（共享 dev DB 不回滚，必须串行，见 README 隔离纪律）
 * - 连本地 ABT 服务（默认 127.0.0.1:8000，由 WEB_PORT 决定）
 */
export default defineConfig({
  testDir: './tests',
  globalSetup: './global-setup.ts',

  // 共享 dev DB 不回滚 → 必须串行，禁止并行污染
  fullyParallel: false,
  workers: 1,

  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,

  reporter: [['html', { open: 'never' }], ['list']],
  expect: { timeout: 10_000 },

  use: {
    baseURL: process.env.ABT_BASE_URL ?? 'http://127.0.0.1:8000',
    storageState: '.auth/admin.json',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    actionTimeout: 10_000,
    navigationTimeout: 15_000,
    ignoreHTTPSErrors: true,
  },

  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
});
