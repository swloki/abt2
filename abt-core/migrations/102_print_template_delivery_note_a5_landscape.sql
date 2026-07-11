-- 送货单默认模板：A5 横向，每页一份完整送货单，每页 ≤7 行明细
--
-- 布局规则：
-- 1. `batch(7)` 把明细分每 7 行一页，每页渲染一个完整 container（标题/客户信息/
--    表格列头/底部注释/签名区全部重复），14 行 → 2 份独立送货单。
-- 2. 除最后一页外，每页 container 加 `page-break-after: always` 强制分页；
--    container 固定 height:136mm（= A5 横向 148mm − 6mm×2 边距），`page-break-inside: avoid`
--    防跨页切割。
-- 3. 每页内部：明细不足 7 行补空行；表格 `table-layout: fixed` 防长名称横向撑爆 +
--    让名称按列宽真正换行；产品名称 `-webkit-line-clamp: 3`、行备注 `line-clamp: 2`
--    超长截断，保证每页 A5 横向一张纸装下。
-- Jinja2 变量与 mock_data::delivery_note_mock / print_shipping 组装的 vars 对齐。
UPDATE print_templates
SET html_content = $tpl$<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>江门市艾伯特照明科技有限公司送货单</title>
    <style>
        @page {
            size: A5 landscape;
            margin: 6mm;
        }
        * { box-sizing: border-box; }
        body {
            font-family: "SimHei", "黑体", "SimSun", "宋体", sans-serif;
            margin: 0;
            color: #000;
            font-size: 11px;
        }
        .container {
            width: 100%;
            height: 136mm;
            margin: 0 auto;
            padding: 0;
            display: flex;
            flex-direction: column;
            page-break-inside: avoid;
        }
        .header {
            display: flex;
            align-items: center;
            justify-content: center;
            margin-bottom: 3px;
            position: relative;
        }
        .logo {
            position: absolute;
            left: 0;
            height: 30px;
        }
        .title {
            font-size: 18px;
            font-weight: bold;
            letter-spacing: 1px;
        }
        .company-info {
            text-align: center;
            font-size: 10px;
            color: #000;
            margin-bottom: 5px;
        }
        .info-grid {
            width: 100%;
            border-collapse: collapse;
            font-size: 11px;
            margin-bottom: 5px;
        }
        .info-grid td {
            padding: 2px 4px;
            vertical-align: middle;
            height: 20px;
        }
        .label {
            text-align: right;
            width: 60px;
            white-space: nowrap;
        }
        .value {
            border-bottom: 1px dashed #000;
            padding-left: 4px;
        }
        .detail-table {
            width: 100%;
            border-collapse: collapse;
            font-size: 11px;
            margin-bottom: 6px;
            flex: 1 1 auto;
            table-layout: fixed;
        }
        .detail-table th, .detail-table td {
            border: 1px solid #000;
            text-align: center;
            padding: 1px 3px;
            height: 20px;
        }
        .detail-table th {
            font-weight: normal;
            background-color: #fff;
        }
        .name-clamp {
            display: -webkit-box;
            -webkit-box-orient: vertical;
            -webkit-line-clamp: 3;
            overflow: hidden;
            word-break: break-word;
            text-align: left;
        }
        .remark-clamp {
            display: -webkit-box;
            -webkit-box-orient: vertical;
            -webkit-line-clamp: 2;
            overflow: hidden;
            word-break: break-word;
            text-align: left;
        }
        .footer-note {
            font-size: 9px;
            line-height: 1.3;
            margin-bottom: 5px;
            border-bottom: 1px dashed #000;
            padding-bottom: 4px;
        }
        .color-lian {
            text-align: center;
            font-size: 10px;
            margin-bottom: 8px;
        }
        .signature-row {
            display: flex;
            justify-content: space-between;
            font-size: 11px;
            padding: 0 4px;
            margin-top: 10px;
        }
        .signature-item {
            width: 20%;
            border-bottom: 1px dashed #000;
            padding-bottom: 2px;
        }
        @media print {
            body { margin: 0; }
            button { display: none; }
        }
    </style>
</head>
<body>

{% for page_items in 明细|batch(7) %}
<div class="container"{% if not loop.last %} style="page-break-after: always;"{% endif %}>
    <div class="header">
        <img class="logo" src="logo.png" alt="艾伯特 LOGO" onerror="this.style.display='none';">
        <div class="title">江门市艾伯特照明科技有限公司送货单</div>
    </div>

    <div class="company-info">
        供货单位：江门市艾伯特照明科技有限公司 &nbsp;&nbsp;
        地址：江门市江海区高新东路19号3幢第五层 &nbsp;&nbsp;
        电话：0750-3868178
    </div>

    <table class="info-grid">
        <tr>
            <td class="label">客户全称：</td>
            <td class="value" style="width: 45%;">{{ 客户全称 }}</td>
            <td class="label">日期：</td>
            <td class="value">{{ 出库日期 }}</td>
        </tr>
        <tr>
            <td class="label">收货人：</td>
            <td class="value">{{ 收货人 }}</td>
            <td class="label">联系电话：</td>
            <td class="value">{{ 联系电话 }}</td>
            <td class="label">客户经理：</td>
            <td class="value">{{ 客户经理 }}</td>
        </tr>
        <tr>
            <td class="label">收货地址：</td>
            <td class="value" colspan="3">{{ 收货地址 }}</td>
            <td class="label">出库单号：</td>
            <td class="value">{{ 出库单号 }}</td>
        </tr>
    </table>

    <table class="detail-table">
        <thead>
            <tr>
                <th style="width: 40px;">序号</th>
                <th style="width: 90px;">产品编码</th>
                <th>产品名称</th>
                <th style="width: 45px;">单位</th>
                <th style="width: 60px;">出库数量</th>
                <th style="width: 180px;">要求</th>
            </tr>
        </thead>
        <tbody>
            {% for item in page_items %}
            <tr>
                <td>{{ loop.index }}</td>
                <td>{{ item.产品编码 }}</td>
                <td style="text-align: left; padding-left: 4px;"><div class="name-clamp">{{ item.产品名称 }}</div></td>
                <td>{{ item.单位 }}</td>
                <td>{{ item.本次出库数量 }}</td>
                <td style="text-align: left; padding-left: 4px;"><div class="remark-clamp">{{ item.行备注 }}</div></td>
            </tr>
            {% endfor %}
            {% set pad = 7 - (page_items|length) %}
            {% if pad > 0 %}
            {% for _ in range(pad) %}
            <tr>
                <td>&nbsp;</td>
                <td></td>
                <td></td>
                <td></td>
                <td></td>
                <td></td>
            </tr>
            {% endfor %}
            {% endif %}
        </tbody>
    </table>

    <div class="footer-note">
        <strong>注：</strong>1、本产品验收后，如有数量问题当面点清。如有质量问题请在七天内书面通知，过期恕不负责。
        2、凡属供方质量问题需退货时，必须按原规格型号退回，否则不接纳。
        3、需方不能以任何借口拖延付款期，否则每天向供方偿付千分之一违约金。
        4、如有争议，由供方当地人民法院受理，并由违约方承担律师费等维权费用。
    </div>

    <div class="color-lian">
        1:白色存根联、2:红色客户联、3:蓝色财务联、4:黄色仓库联。
    </div>

    <div class="signature-row">
        <div class="signature-item">制单人：</div>
        <div class="signature-item">业务员：</div>
        <div class="signature-item">发货人：</div>
        <div class="signature-item" style="width: 25%;">收货单位签名：</div>
    </div>
</div>
{% endfor %}

</body>
</html>$tpl$,
    updated_at = NOW()
WHERE document_type = 'delivery_note' AND is_default = TRUE;
