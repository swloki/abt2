import { test } from '@playwright/test';
import { assertLoggedIn } from '../fixtures/auth';

/**
 * 跨域路由 smoke：逐个 goto，断言登录态 + admin 外壳渲染。
 * 纯 GET，无副作用 —— 主力高频回归，随便跑。
 *
 * 路由清单来自 abt-web/src/routes/（TypedPath::PATH），覆盖 sales/master_data/purchase/wms/mes/system。
 * 新增页面时往 ROUTES 加一行即可。
 */
const ROUTES: readonly string[] = [
  '/admin',
  '/admin/customers',
  '/admin/quotations',
  '/admin/orders',
  '/admin/wms/shipping',
  '/admin/md/products',
  '/admin/md/boms',
  '/admin/purchase/orders',
  '/admin/purchase/payments',
  '/admin/wms/stock',
  '/admin/wms/stock-in',
  '/admin/wms/bins',
  '/admin/wms/conversions',
  '/admin/wms/work-center',
  '/admin/wms/ledger',
  '/admin/mes/work-center',
  '/admin/mes/demand-pool',
  '/admin/system/users',
];

for (const url of ROUTES) {
  test(`smoke: ${url} 可达且已登录`, async ({ page }) => {
    await page.goto(url);
    await assertLoggedIn(page);
  });
}
