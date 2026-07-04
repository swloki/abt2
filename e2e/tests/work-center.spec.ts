import { test, expect } from '@playwright/test';

/**
 * WMS 作业中心 — 5 tab 切换 + 紧急度筛选。
 * 纯 GET，无副作用。验证 HTMX 局部刷新机制（点 tab → #wc-domain-card outerHTML 替换 + URL 不变）。
 *
 * 事实：
 * - 唯一端点 GET /admin/wms/work-center，按 ?domain= 切换（wms_work_center.rs:1241 render_domain_card）
 * - tab 由标准 status_tabs 组件渲染（#status-tabs），label = 待收货/待出库/待领料/待调拨/待盘点
 * - #wc-domain-card 是替换边界（hx-target="this" hx-select="#wc-domain-card" hx-swap="outerHTML"）
 * - 列表页禁 hx-push-url（htmx-patterns.md §2.3）→ 切 tab URL 不变
 */
const TABS = [
  { domain: 'arrival', label: '待收货' },
  { domain: 'outbound', label: '待出库' },
  { domain: 'requisition', label: '待领料' },
  { domain: 'transfer', label: '待调拨' },
  { domain: 'cycle-count', label: '待盘点' },
] as const;

test.describe('WMS 作业中心 — 5 tab 切换', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/admin/wms/work-center');
    await expect(page.locator('#wc-domain-card')).toBeVisible();
  });

  for (const { label } of TABS) {
    test(`切到「${label}」：#wc-domain-card 局部刷新 + URL 不变`, async ({ page }) => {
      const urlBefore = page.url();
      const tab = page.locator('#status-tabs a', { hasText: label }).first();

      // 点 tab → 等 HTMX GET /admin/wms/work-center 响应到达
      await Promise.all([
        page.waitForResponse(
          (r) => r.url().includes('/admin/wms/work-center') && r.request().method() === 'GET',
        ),
        tab.click(),
      ]);

      // #wc-domain-card 被 outerHTML 替换后仍在；URL 不变（列表页禁 push-url）
      await expect(page.locator('#wc-domain-card')).toBeVisible();
      expect(page.url()).toBe(urlBefore);
    });
  }

  test('紧急度筛选「逾期」触发局部刷新 + URL 不变', async ({ page }) => {
    const urlBefore = page.url();
    const select = page.locator('#wc-urgency-select');
    await expect(select).toBeVisible();

    await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/admin/wms/work-center') && r.request().method() === 'GET',
      ),
      select.selectOption('overdue'),
    ]);

    await expect(page.locator('#wc-domain-card')).toBeVisible();
    expect(page.url()).toBe(urlBefore);
    // 选中项保持
    await expect(select).toHaveValue('overdue');
  });

  test('关键词搜索触发局部刷新（防抖）', async ({ page }) => {
    const urlBefore = page.url();
    const input = page.locator('.wc-search-input');

    // HTMX 监听 `keyup changed delay:300ms from:.wc-search-input`（wms_work_center.rs:1310）
    // fill() 只触发 input/change、不发 keyup → 必须手动 dispatch keyup 才能触发搜索
    await input.fill('E2E-NO-SUCH-DOC');
    await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/admin/wms/work-center') && r.request().method() === 'GET',
      ),
      input.dispatchEvent('keyup'), // → HTMX 防抖 300ms → GET
    ]);

    await expect(page.locator('#wc-domain-card')).toBeVisible();
    expect(page.url()).toBe(urlBefore);
  });
});
