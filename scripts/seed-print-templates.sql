-- Seed quotation / sales_order / purchase_order 默认打印模板
-- 风格对齐 delivery_note（同 CSS 骨架），字段对齐 mock_data.rs 的 vars key
BEGIN;

INSERT INTO print_templates (name, document_type, description, html_content, is_default, created_at)
VALUES
('标准报价单', 'quotation', '系统预置默认模板（可克隆后自定义）', $q$
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>报价单</title>
<style>
body { font-family: "SimHei","黑体","SimSun","宋体",sans-serif; margin: 20px; color: #000; }
.container { width: 900px; margin: 0 auto; border: 1px dashed #000; padding: 20px; box-sizing: border-box; }
.header { text-align: center; margin-bottom: 5px; }
.title { font-size: 24px; font-weight: bold; letter-spacing: 2px; }
.company-info { text-align: center; font-size: 12px; color: #000; margin-bottom: 15px; }
.info-table { width: 100%; border-collapse: collapse; font-size: 14px; margin-bottom: 10px; table-layout: fixed; }
.info-table td { padding: 6px 2px; vertical-align: middle; }
.label { text-align: right; width: 90px; white-space: nowrap; }
.value { border-bottom: 1px dashed #000; padding-left: 5px; text-align: left; }
.detail-table { width: 100%; border-collapse: collapse; font-size: 14px; margin-bottom: 15px; }
.detail-table th, .detail-table td { border: 1px solid #000; text-align: center; padding: 6px 4px; height: 28px; }
.detail-table th { font-weight: normal; background-color: #fff; }
.total-row { text-align: right; font-size: 16px; font-weight: bold; margin: 10px 0; }
.note { font-size: 12px; color: #000; line-height: 1.6; margin: 8px 0; border-bottom: 1px dashed #000; padding-bottom: 8px; }
.signature-row { display: flex; justify-content: space-between; font-size: 14px; padding: 0 10px; margin-top: 30px; }
.signature-item { width: 22%; border-bottom: 1px dashed #000; padding-bottom: 2px; }
@media print { body { margin: 0; } .container { border: none; width: 100%; padding: 0; } }
</style>
</head>
<body>
<div class="container">
  <div class="header"><div class="title">{{ 公司名称 }}</div></div>
  <div class="company-info">报价单</div>
  <table class="info-table">
    <colgroup><col style="width:90px"><col><col style="width:90px"><col></colgroup>
    <tr><td class="label">报价单号：</td><td class="value">{{ 报价单号 }}</td><td class="label">报价日期：</td><td class="value">{{ 报价日期 }}</td></tr>
    <tr><td class="label">客户全称：</td><td class="value" colspan="3">{{ 客户全称 }}</td></tr>
    <tr><td class="label">有效期至：</td><td class="value">{{ 有效期至 }}</td><td class="label">销售员：</td><td class="value">{{ 销售员 }}</td></tr>
    <tr><td class="label">付款条款：</td><td class="value" colspan="3">{{ 付款条款 }}</td></tr>
  </table>
  <table class="detail-table">
    <thead><tr><th style="width:50px">序号</th><th>产品名称</th><th style="width:80px">数量</th><th style="width:60px">单位</th><th style="width:90px">单价</th><th style="width:70px">折扣率</th><th style="width:100px">金额</th></tr></thead>
    <tbody>
      {% for item in 明细 %}
      <tr><td>{{ loop.index }}</td><td style="text-align:left;padding-left:5px">{{ item.产品名称 }}</td><td>{{ item.数量 }}</td><td>{{ item.单位 }}</td><td>{{ item.单价 }}</td><td>{{ item.折扣率 }}</td><td>{{ item.金额 }}</td></tr>
      {% endfor %}
    </tbody>
  </table>
  <div class="total-row">报价总金额（含税）：¥{{ 报价总金额 }}</div>
  <div class="note"><strong>备注：</strong>{{ 交货条款 }}</div>
  <div class="signature-row"><div class="signature-item">制单人：</div><div class="signature-item">业务员：</div><div class="signature-item">客户签字：</div><div class="signature-item">日期：</div></div>
</div>
</body>
</html>
$q$, true, NOW()),

('标准销售订单', 'sales_order', '系统预置默认模板（可克隆后自定义）', $q$
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>销售订单</title>
<style>
body { font-family: "SimHei","黑体","SimSun","宋体",sans-serif; margin: 20px; color: #000; }
.container { width: 900px; margin: 0 auto; border: 1px dashed #000; padding: 20px; box-sizing: border-box; }
.header { text-align: center; margin-bottom: 5px; }
.title { font-size: 24px; font-weight: bold; letter-spacing: 2px; }
.company-info { text-align: center; font-size: 12px; color: #000; margin-bottom: 15px; }
.info-table { width: 100%; border-collapse: collapse; font-size: 14px; margin-bottom: 10px; table-layout: fixed; }
.info-table td { padding: 6px 2px; vertical-align: middle; }
.label { text-align: right; width: 90px; white-space: nowrap; }
.value { border-bottom: 1px dashed #000; padding-left: 5px; text-align: left; }
.detail-table { width: 100%; border-collapse: collapse; font-size: 14px; margin-bottom: 15px; }
.detail-table th, .detail-table td { border: 1px solid #000; text-align: center; padding: 6px 4px; height: 28px; }
.detail-table th { font-weight: normal; background-color: #fff; }
.total-row { text-align: right; font-size: 16px; font-weight: bold; margin: 10px 0; }
.note { font-size: 12px; color: #000; line-height: 1.6; margin: 8px 0; border-bottom: 1px dashed #000; padding-bottom: 8px; }
.signature-row { display: flex; justify-content: space-between; font-size: 14px; padding: 0 10px; margin-top: 30px; }
.signature-item { width: 22%; border-bottom: 1px dashed #000; padding-bottom: 2px; }
@media print { body { margin: 0; } .container { border: none; width: 100%; padding: 0; } }
</style>
</head>
<body>
<div class="container">
  <div class="header"><div class="title">{{ 公司名称 }}</div></div>
  <div class="company-info">销售订单</div>
  <table class="info-table">
    <colgroup><col style="width:90px"><col><col style="width:90px"><col></colgroup>
    <tr><td class="label">订单号：</td><td class="value">{{ 订单号 }}</td><td class="label">订单日期：</td><td class="value">{{ 订单日期 }}</td></tr>
    <tr><td class="label">客户全称：</td><td class="value" colspan="3">{{ 客户全称 }}</td></tr>
    <tr><td class="label">交货地址：</td><td class="value" colspan="3">{{ 交货地址 }}</td></tr>
    <tr><td class="label">销售员：</td><td class="value">{{ 销售员 }}</td><td class="label">订单状态：</td><td class="value">{{ 订单状态 }}</td></tr>
  </table>
  <table class="detail-table">
    <thead><tr><th style="width:50px">序号</th><th>产品名称</th><th style="width:80px">数量</th><th style="width:60px">单位</th><th style="width:90px">单价</th><th style="width:100px">金额</th><th style="width:80px">已发数量</th><th style="width:80px">未交数量</th></tr></thead>
    <tbody>
      {% for item in 明细 %}
      <tr><td>{{ loop.index }}</td><td style="text-align:left;padding-left:5px">{{ item.产品名称 }}</td><td>{{ item.数量 }}</td><td>{{ item.单位 }}</td><td>{{ item.单价 }}</td><td>{{ item.金额 }}</td><td>{{ item.已发数量 }}</td><td>{{ item.未交数量 }}</td></tr>
      {% endfor %}
    </tbody>
  </table>
  <div class="total-row">订单总金额（含税）：¥{{ 订单总金额 }}</div>
  <div class="note"><strong>付款条款：</strong>{{ 付款条款 }}</div>
  <div class="signature-row"><div class="signature-item">制单人：</div><div class="signature-item">业务员：</div><div class="signature-item">客户签字：</div><div class="signature-item">日期：</div></div>
</div>
</body>
</html>
$q$, true, NOW()),

('标准采购订单', 'purchase_order', '系统预置默认模板（可克隆后自定义）', $q$
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>采购订单</title>
<style>
body { font-family: "SimHei","黑体","SimSun","宋体",sans-serif; margin: 20px; color: #000; }
.container { width: 900px; margin: 0 auto; border: 1px dashed #000; padding: 20px; box-sizing: border-box; }
.header { text-align: center; margin-bottom: 5px; }
.title { font-size: 24px; font-weight: bold; letter-spacing: 2px; }
.company-info { text-align: center; font-size: 12px; color: #000; margin-bottom: 15px; }
.info-table { width: 100%; border-collapse: collapse; font-size: 14px; margin-bottom: 10px; table-layout: fixed; }
.info-table td { padding: 6px 2px; vertical-align: middle; }
.label { text-align: right; width: 90px; white-space: nowrap; }
.value { border-bottom: 1px dashed #000; padding-left: 5px; text-align: left; }
.detail-table { width: 100%; border-collapse: collapse; font-size: 14px; margin-bottom: 15px; }
.detail-table th, .detail-table td { border: 1px solid #000; text-align: center; padding: 6px 4px; height: 28px; }
.detail-table th { font-weight: normal; background-color: #fff; }
.total-row { text-align: right; font-size: 16px; font-weight: bold; margin: 10px 0; }
.note { font-size: 12px; color: #000; line-height: 1.6; margin: 8px 0; border-bottom: 1px dashed #000; padding-bottom: 8px; }
.signature-row { display: flex; justify-content: space-between; font-size: 14px; padding: 0 10px; margin-top: 30px; }
.signature-item { width: 22%; border-bottom: 1px dashed #000; padding-bottom: 2px; }
@media print { body { margin: 0; } .container { border: none; width: 100%; padding: 0; } }
</style>
</head>
<body>
<div class="container">
  <div class="header"><div class="title">{{ 公司名称 }}</div></div>
  <div class="company-info">采购订单</div>
  <table class="info-table">
    <colgroup><col style="width:90px"><col><col style="width:90px"><col></colgroup>
    <tr><td class="label">采购单号：</td><td class="value">{{ 采购单号 }}</td><td class="label">采购日期：</td><td class="value">{{ 采购日期 }}</td></tr>
    <tr><td class="label">供应商：</td><td class="value" colspan="3">{{ 供应商全称 }}</td></tr>
    <tr><td class="label">采购员：</td><td class="value">{{ 采购员 }}</td><td class="label">采购状态：</td><td class="value">{{ 采购状态 }}</td></tr>
    <tr><td class="label">付款条款：</td><td class="value" colspan="3">{{ 付款条款 }}</td></tr>
  </table>
  <table class="detail-table">
    <thead><tr><th style="width:50px">序号</th><th>产品名称</th><th style="width:90px">数量</th><th style="width:60px">单位</th><th style="width:90px">单价</th><th style="width:110px">金额</th><th style="width:90px">已收数量</th></tr></thead>
    <tbody>
      {% for item in 明细 %}
      <tr><td>{{ loop.index }}</td><td style="text-align:left;padding-left:5px">{{ item.产品名称 }}</td><td>{{ item.数量 }}</td><td>{{ item.单位 }}</td><td>{{ item.单价 }}</td><td>{{ item.金额 }}</td><td>{{ item.已收数量 }}</td></tr>
      {% endfor %}
    </tbody>
  </table>
  <div class="total-row">采购总金额（含税）：¥{{ 采购总金额 }}</div>
  <div class="signature-row"><div class="signature-item">制单人：</div><div class="signature-item">采购员：</div><div class="signature-item">供应商签字：</div><div class="signature-item">日期：</div></div>
</div>
</body>
</html>
$q$, true, NOW());

COMMIT;
