-- 默认送货单模板（minijinja / Jinja2 语法，中文变量名）
-- 单值变量：{{ 客户全称 }} {{ 出库日期 }} {{ 收货人 }} {{ 联系电话 }} {{ 客户经理 }} {{ 收货地址 }} {{ 出库单号 }}
-- 明细循环：{% for item in 明细 %} ... {{ item.产品编码 }} {{ item.产品名称 }} {{ item.单位 }}
--           {{ item.本次出库数量 }} {{ item.行备注 }} ... {% endfor %}
-- 渲染上下文与 mock_data::delivery_note_mock 对齐，预览可直接渲染。
INSERT INTO print_templates (name, document_type, description, html_content, is_default, created_at)
VALUES (
    '标准送货单',
    'delivery_note',
    '系统预置标准送货单模板（Jinja2 语法）',
    $tpl$<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>江门市艾伯特照明科技有限公司送货单</title>
    <style>
        body {
            font-family: "SimHei", "黑体", "SimSun", "宋体", sans-serif;
            margin: 20px;
            color: #000;
        }
        .container {
            width: 900px;
            margin: 0 auto;
            border: 1px dashed #000;
            padding: 20px;
            box-sizing: border-box;
        }
        .header {
            display: flex;
            align-items: center;
            justify-content: center;
            margin-bottom: 5px;
            position: relative;
        }
        .logo {
            position: absolute;
            left: 10px;
            height: 45px;
        }
        .title {
            font-size: 24px;
            font-weight: bold;
            letter-spacing: 2px;
        }
        .company-info {
            text-align: center;
            font-size: 12px;
            color: #000;
            margin-bottom: 15px;
        }
        .info-grid {
            width: 100%;
            border-collapse: collapse;
            font-size: 14px;
            margin-bottom: 10px;
        }
        .info-grid td {
            padding: 4px 5px;
            vertical-align: middle;
        }
        .label {
            text-align: right;
            width: 80px;
            white-space: nowrap;
        }
        .value {
            border-bottom: 1px dashed #000;
            padding-left: 5px;
        }
        .detail-table {
            width: 100%;
            border-collapse: collapse;
            font-size: 14px;
            margin-bottom: 15px;
        }
        .detail-table th, .detail-table td {
            border: 1px solid #000;
            text-align: center;
            padding: 6px 4px;
            height: 28px;
        }
        .detail-table th {
            font-weight: normal;
            background-color: #fff;
        }
        .footer-note {
            font-size: 12px;
            line-height: 1.6;
            margin-bottom: 15px;
            border-bottom: 1px dashed #000;
            padding-bottom: 10px;
        }
        .color-lian {
            text-align: center;
            font-size: 13px;
            margin-bottom: 20px;
        }
        .signature-row {
            display: flex;
            justify-content: space-between;
            font-size: 14px;
            padding: 0 10px;
            margin-top: 25px;
        }
        .signature-item {
            width: 20%;
            border-bottom: 1px dashed #000;
            padding-bottom: 2px;
        }
        @media print {
            body { margin: 0; }
            .container { border: none; width: 100%; }
            button { display: none; }
        }
    </style>
</head>
<body>

<div class="container">
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
                <th style="width: 50px;">序号</th>
                <th style="width: 120px;">产品编码</th>
                <th>产品名称</th>
                <th style="width: 60px;">单位</th>
                <th style="width: 80px;">出库数量</th>
                <th style="width: 250px;">要求</th>
            </tr>
        </thead>
        <tbody>
            {% for item in 明细 %}
            <tr>
                <td>{{ loop.index }}</td>
                <td>{{ item.产品编码 }}</td>
                <td style="text-align: left; padding-left: 5px;">{{ item.产品名称 }}</td>
                <td>{{ item.单位 }}</td>
                <td>{{ item.本次出库数量 }}</td>
                <td style="text-align: left; padding-left: 5px;">{{ item.行备注 }}</td>
            </tr>
            {% endfor %}
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

</body>
</html>$tpl$,
    TRUE,
    NOW()
)
ON CONFLICT DO NOTHING;
