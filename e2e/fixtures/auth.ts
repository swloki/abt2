import { expect, type Page } from '@playwright/test';

/** globalSetup 持久化的登录态文件（cookie id=...） */
export const STORAGE_STATE = '.auth/admin.json';

/**
 * 断言当前 page 处于登录态：
 * 没被 auth_middleware 踢回 /login，且 admin 外壳已渲染。
 * （未登录访问 /admin/... 会被 302 到 /login）
 */
export async function assertLoggedIn(page: Page): Promise<void> {
  await expect(page).not.toHaveURL(/\/login(\?|$)/);
  await expect(page.locator('#app-wrapper')).toBeVisible();
}
