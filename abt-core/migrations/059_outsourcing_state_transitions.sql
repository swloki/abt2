-- OutsourcingOrder 状态机定义
-- 之前完全缺失：state_definitions / state_transition_defs 中无 OutsourcingOrder 记录，
-- 导致 send/receive/cancel/convert 的 transition() 全部报错"状态转换无效:  -> X"。
-- 对照 docs/uml-design/05-outsourcing.html 状态机补齐。

-- 状态定义
INSERT INTO state_definitions (entity_type, state_name, label, is_initial, is_final) VALUES
    ('OutsourcingOrder', 'Draft',               '草稿',     true,  false),
    ('OutsourcingOrder', 'Sent',                '已发送',   false, false),
    ('OutsourcingOrder', 'InProduction',        '生产中',   false, false),
    ('OutsourcingOrder', 'Delivered',           '已交付',   false, false),
    ('OutsourcingOrder', 'Received',            '已收货',   false, false),
    ('OutsourcingOrder', 'Closed',              '已关闭',   false, true),
    ('OutsourcingOrder', 'ConvertedToInternal', '已转自制', false, true),
    ('OutsourcingOrder', 'Cancelled',           '已取消',   false, true)
ON CONFLICT DO NOTHING;

-- 转换规则（from_state '' 表示初始状态）
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('OutsourcingOrder', '',         'Draft',               NULL, 1),
    ('OutsourcingOrder', 'Draft',    'Sent',                NULL, 2),
    ('OutsourcingOrder', 'Sent',     'Received',            NULL, 3),
    ('OutsourcingOrder', 'Received', 'Closed',              NULL, 4),
    ('OutsourcingOrder', 'Draft',    'ConvertedToInternal', NULL, 5),
    ('OutsourcingOrder', 'Sent',     'ConvertedToInternal', NULL, 6),
    ('OutsourcingOrder', 'Draft',    'Cancelled',           NULL, 7)
ON CONFLICT DO NOTHING;

-- 委外仓标记为虚拟仓库（委外发料/收货的虚拟仓语义）。
-- 修复创建表单"虚拟仓库"下拉只列 is_virtual=true 的仓库、而 DB 全为 false 导致无选项、无法创建委外单。
UPDATE warehouses SET is_virtual = true WHERE name = '委外仓' AND is_virtual = false;
