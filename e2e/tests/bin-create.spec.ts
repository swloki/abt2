import { test, expect } from '@playwright/test';
import { WAREHOUSE_A, uniqueTag } from '../fixtures/wms';

/**
 * 库位创建（写操作示范 spec / 模板）。
 * 验证：导航 → 表单填写 → 仓库/库区级联下拉 → HTMX 提交 → HX-Redirect 详情页 → 列表可见。
 * 隔离：每次用唯一 code（uniqueTag → T-PLAY-BIN-{timestamp}），dev DB 不回滚。
 *
 * 事实（wms_bin_create.rs）：
 * - form id=bin-create-form，hx-post=/admin/wms/bins/create，hx-swap=none（:122）
 * - 仓库→库区级联：select#warehouse-select onchange=updateZones() → 库区 option 按 data-wh 显隐（:266-279）
 * - 提交成功 → HX-Redirect: /admin/wms/bins/{id}（:102-103）
 * 列表（wms_bin_list.rs）：#bin-data-card（:152），支持 ?code= 查询（BinQueryParams.code）
 */
test.describe('WMS 库位创建（写操作）', () => {
  test('新建库位端到端：填表 → 提交 → 跳详情页 → 列表可见', async ({ page }) => {
    const code = uniqueTag('T-PLAY-BIN');
    const name = `E2E测试库位 ${code}`;

    // 1. 打开创建页
    await page.goto('/admin/wms/bins/create');
    await expect(page.locator('#bin-create-form')).toBeVisible();

    // 2. 选仓库（WAREHOUSE_A）→ 触发 updateZones() 刷新库区下拉
    await page.locator('#warehouse-select').selectOption(String(WAREHOUSE_A));

    // 3. 选该仓库下第一个库区（不硬编码 zone id，从级联 option 动态取）
    const zoneOption = page.locator(`#zone-select option[data-wh="${WAREHOUSE_A}"]`).first();
    const zoneId = await zoneOption.getAttribute('value');
    expect(zoneId, `仓库 ${WAREHOUSE_A} 下应有至少一个库区`).not.toBeNull();
    await page.locator('#zone-select').selectOption(zoneId!);

    // 4. 填编码 + 名称（唯一后缀防共享 dev DB 撞码）
    await page.locator('input[name="code"]').fill(code);
    await page.locator('input[name="name"]').fill(name);

    // 5. 提交 → 等 HX-Redirect 整页跳到详情页 /admin/wms/bins/{id}
    await Promise.all([
      page.waitForURL(/\/admin\/wms\/bins\/\d+/, { timeout: 15_000 }),
      page.getByRole('button', { name: '保存库位' }).click(),
    ]);

    // 6. 详情页渲染（外壳 + 编码可见 = 创建成功）
    await expect(page.locator('#app-wrapper')).toBeVisible();
    await expect(page.locator('body')).toContainText(code);

    // 7. 回列表按 code 搜，断言新库位在
    await page.goto(`/admin/wms/bins?code=${encodeURIComponent(code)}`);
    await expect(page.locator('#bin-data-card')).toBeVisible();
    await expect(page.locator('#bin-data-card')).toContainText(code);
  });
});
