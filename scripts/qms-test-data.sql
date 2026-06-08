-- ============================================================================
-- QMS Test Data — 质量管理模块测试数据
-- Dependencies: products (565+), customers (1,3,7)
-- ============================================================================

BEGIN;

-- Clear existing QMS data
DELETE FROM rmas WHERE doc_number LIKE 'TEST-QMS-%';
DELETE FROM mrbs WHERE doc_number LIKE 'TEST-QMS-%';
DELETE FROM inspection_results WHERE doc_number LIKE 'TEST-QMS-%';
DELETE FROM inspection_specifications WHERE doc_number LIKE 'TEST-QMS-%';

-- ============================================================================
-- 1. Inspection Specifications (检验规格) — 8 records
-- ============================================================================

INSERT INTO inspection_specifications (doc_number, product_id, inspection_type, check_items, sample_plan, status, version, operator_id) VALUES
-- IQC 来料检验
('TEST-QMS-SPEC-001', 565, 1,
 '[{"item":"外观检查","standard":"无划痕、无变形","tolerance":"—","method":"目视"},{"item":"电参数测试","standard":"正向电压≤3.2V","tolerance":"±0.1V","method":"积分球"},{"item":"光通量测试","standard":"≥50lm","tolerance":"—","method":"分布光度计"},{"item":"色温检测","standard":"6000-6500K","tolerance":"±200K","method":"光谱仪"},{"item":"焊接拉力测试","standard":"≥1.5N","tolerance":"—","method":"拉力计"}]',
 '{"level":"II","aql":"1.0","mode":"Normal"}',
 2, 1),
('TEST-QMS-SPEC-002', 568, 1,
 '[{"item":"外观检查","standard":"无毛刺、无变色","tolerance":"—","method":"目视"},{"item":"尺寸测量","standard":"符合图纸","tolerance":"±0.2mm","method":"卡尺"},{"item":"插拔力测试","standard":"3-8N","tolerance":"—","method":"推拉力计"}]',
 '{"level":"I","aql":"1.5","mode":"Normal"}',
 2, 1),
-- IPQC 过程检验
('TEST-QMS-SPEC-003', 569, 2,
 '[{"item":"锡膏厚度","standard":"0.12mm","tolerance":"±0.02mm","method":"锡膏测厚仪"},{"item":"贴片偏移","standard":"≤0.1mm","tolerance":"—","method":"AOI"},{"item":"回流温度曲线","standard":"符合标准曲线","tolerance":"±5°C","method":"炉温测试仪"},{"item":"X-Ray检测","standard":"无虚焊、连锡","tolerance":"—","method":"X-Ray"}]',
 '{"level":"II","aql":"0.65","mode":"Normal"}',
 2, 1),
-- FQC 终检
('TEST-QMS-SPEC-004', 565, 3,
 '[{"item":"电性能测试","standard":"输入电压范围100-240V","tolerance":"—","method":"安规综合测试仪"},{"item":"耐压测试","standard":"3000V/1mA/1s","tolerance":"—","method":"耐压测试仪"},{"item":"接地电阻","standard":"≤0.1Ω","tolerance":"—","method":"接地电阻测试仪"},{"item":"老化测试","standard":"满载4h无异常","tolerance":"—","method":"老化架"},{"item":"外观检查","standard":"无划痕、标识清晰","tolerance":"—","method":"目视"}]',
 '{"level":"III","aql":"0.25","mode":"Tightened"}',
 2, 1),
-- OQC 出货检验
('TEST-QMS-SPEC-005', 565, 4,
 '[{"item":"包装完整性","standard":"无破损","tolerance":"—","method":"目视"},{"item":"标签核对","standard":"与订单一致","tolerance":"—","method":"核对"},{"item":"数量清点","standard":"与装箱单一致","tolerance":"—","method":"称重+计数"}]',
 '{"level":"II","aql":"2.5","mode":"Normal"}',
 2, 1),
-- Draft 草稿状态
('TEST-QMS-SPEC-006', 569, 2,
 '[{"item":"电压测试","standard":"输出12V±5%","tolerance":"±0.6V","method":"万用表"}]',
 '{"level":"II","aql":"1.0","mode":"Normal"}',
 1, 1),
-- 停用状态
('TEST-QMS-SPEC-007', 568, 4,
 '[{"item":"外观","standard":"无缺陷","tolerance":"—","method":"目视"}]',
 '{"level":"I","aql":"2.5","mode":"Normal"}',
 3, 1),
-- 更多 Active
('TEST-QMS-SPEC-008', 569, 3,
 '[{"item":"功率测试","standard":"400W±5%","tolerance":"±20W","method":"功率计"},{"item":"温升测试","standard":"≤65°C","tolerance":"—","method":"红外测温仪"},{"item":"绝缘电阻","standard":"≥2MΩ","tolerance":"—","method":"绝缘电阻测试仪"}]',
 '{"level":"II","aql":"1.0","mode":"Normal"}',
 2, 1);

-- ============================================================================
-- 2. Inspection Results (检验结果) — 10 records
-- ============================================================================

INSERT INTO inspection_results (doc_number, spec_id, source_type, source_id, inspection_type, batch_no, sample_qty, qualified_qty, unqualified_qty, result, check_results, inspector_id, inspection_date, status, operator_id) VALUES
-- IQC 合格
('TEST-QMS-RES-001', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-001'), 1, 1, 1, 'B2026060801', 50, 50, 0, 1,
 '[{"item":"外观检查","measured":"合格","pass":true,"remark":""},{"item":"电参数测试","measured":"3.15V","pass":true,"remark":""},{"item":"光通量测试","measured":"52lm","pass":true,"remark":""},{"item":"色温检测","measured":"6200K","pass":true,"remark":""},{"item":"焊接拉力测试","measured":"2.1N","pass":true,"remark":""}]',
 1, '2026-06-08', 2, 1),
-- IQC 不合格
('TEST-QMS-RES-002', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-002'), 1, 2, 1, 'B2026060802', 40, 36, 4, 2,
 '[{"item":"外观检查","measured":"4件毛刺","pass":false,"remark":"4件存在毛刺"},{"item":"尺寸测量","measured":"合格","pass":true,"remark":""},{"item":"插拔力测试","measured":"5.2N","pass":true,"remark":""}]',
 1, '2026-06-07', 3, 1),
-- IQC 让步接收
('TEST-QMS-RES-003', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-001'), 1, 3, 1, 'B2026060803', 80, 78, 2, 3,
 '[{"item":"外观检查","measured":"合格","pass":true,"remark":""},{"item":"电参数测试","measured":"3.25V","pass":true,"remark":"略高但可接收"},{"item":"光通量测试","measured":"51lm","pass":true,"remark":""},{"item":"色温检测","measured":"6350K","pass":true,"remark":""},{"item":"焊接拉力测试","measured":"1.8N","pass":true,"remark":""}]',
 1, '2026-06-06', 3, 1),
-- IPQC 合格
('TEST-QMS-RES-004', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-003'), 2, 1, 2, 'B2026060601', 30, 30, 0, 1,
 '[{"item":"锡膏厚度","measured":"0.13mm","pass":true,"remark":""},{"item":"贴片偏移","measured":"0.05mm","pass":true,"remark":""},{"item":"回流温度曲线","measured":"合格","pass":true,"remark":""},{"item":"X-Ray检测","measured":"合格","pass":true,"remark":""}]',
 1, '2026-06-06', 2, 1),
-- IPQC 不合格
('TEST-QMS-RES-005', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-003'), 2, 2, 2, 'B2026060501', 25, 23, 2, 2,
 '[{"item":"锡膏厚度","measured":"0.18mm","pass":false,"remark":"偏厚"},{"item":"贴片偏移","measured":"0.3mm","pass":false,"remark":"U1偏移超规格"},{"item":"回流温度曲线","measured":"合格","pass":true,"remark":""},{"item":"X-Ray检测","measured":"2片虚焊","pass":false,"remark":""}]',
 1, '2026-06-05', 2, 1),
-- FQC 合格
('TEST-QMS-RES-006', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-004'), 2, 3, 3, 'B2026060401', 100, 100, 0, 1,
 '[{"item":"电性能测试","measured":"220V输入正常","pass":true,"remark":""},{"item":"耐压测试","measured":"合格","pass":true,"remark":""},{"item":"接地电阻","measured":"0.05Ω","pass":true,"remark":""},{"item":"老化测试","measured":"合格","pass":true,"remark":""},{"item":"外观检查","measured":"合格","pass":true,"remark":""}]',
 1, '2026-06-04', 3, 1),
-- FQC 不合格
('TEST-QMS-RES-007', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-004'), 2, 4, 3, 'B2026060301', 60, 55, 5, 2,
 '[{"item":"电性能测试","measured":"输出偏差6%","pass":false,"remark":"超出规格±5%"},{"item":"耐压测试","measured":"合格","pass":true,"remark":""},{"item":"接地电阻","measured":"0.08Ω","pass":true,"remark":""},{"item":"老化测试","measured":"3台变色","pass":false,"remark":""},{"item":"外观检查","measured":"2台划痕","pass":false,"remark":""}]',
 1, '2026-06-03', 2, 1),
-- OQC 合格
('TEST-QMS-RES-008', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-005'), 3, 1, 4, 'B2026060201', 20, 20, 0, 1,
 '[{"item":"包装完整性","measured":"合格","pass":true,"remark":""},{"item":"标签核对","measured":"一致","pass":true,"remark":""},{"item":"数量清点","measured":"一致","pass":true,"remark":""}]',
 1, '2026-06-02', 3, 1),
-- OQC 不合格
('TEST-QMS-RES-009', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-005'), 3, 2, 4, 'B2026060202', 20, 18, 2, 2,
 '[{"item":"包装完整性","measured":"2箱破损","pass":false,"remark":""},{"item":"标签核对","measured":"一致","pass":true,"remark":""},{"item":"数量清点","measured":"一致","pass":true,"remark":""}]',
 1, '2026-06-02', 2, 1),
-- Pending 待检验
('TEST-QMS-RES-010', (SELECT id FROM inspection_specifications WHERE doc_number='TEST-QMS-SPEC-001'), 1, 4, 1, 'B2026060901', 50, 0, 0, 1, '[]', 0, NULL, 1, 1);

-- ============================================================================
-- 3. MRB (不良评审) — 5 records
-- ============================================================================

INSERT INTO mrbs (doc_number, inspection_result_id, product_id, defect_description, disposition, responsible_party, cost_impact, status, remark, operator_id) VALUES
('TEST-QMS-MRB-001',
 (SELECT id FROM inspection_results WHERE doc_number='TEST-QMS-RES-002'),
 568, '来料检验发现AC插座4件存在毛刺缺陷，表面处理不良', 4, 2, 280.0000, 2, '供应商责任，已通知改善', 1),
('TEST-QMS-MRB-002',
 (SELECT id FROM inspection_results WHERE doc_number='TEST-QMS-RES-005'),
 569, 'IPQC巡检发现SMT贴片偏移，IC U1位置偏移量0.3mm超规格，2片虚焊', 4, 1, 860.0000, 1, '内部制程问题，需调整贴装参数', 1),
('TEST-QMS-MRB-003',
 (SELECT id FROM inspection_results WHERE doc_number='TEST-QMS-RES-007'),
 565, 'FQC发现LED驱动板输出电压偏差超出规格±5%，批次不良率8.3%', 2, 2, 5400.0000, 4, '已退货给供应商', 1),
('TEST-QMS-MRB-004',
 (SELECT id FROM inspection_results WHERE doc_number='TEST-QMS-RES-009'),
 565, '出货检验发现2箱包装破损，内部产品外壳有刮痕', 3, 1, 2150.0000, 3, '降级处理，折价销售', 1),
('TEST-QMS-MRB-005',
 (SELECT id FROM inspection_results WHERE doc_number='TEST-QMS-RES-005'),
 569, '老化测试后外壳变色，耐温性能不达标', 1, 1, 1280.0000, 2, '报废处理', 1);

-- ============================================================================
-- 4. RMA (客诉追溯) — 4 records
-- ============================================================================

INSERT INTO rmas (doc_number, customer_id, product_id, defect_description, severity, root_cause, corrective_action, status, remark, operator_id) VALUES
('TEST-QMS-RMA-001',
 7, 565, '客户反馈产品在使用2周后出现闪烁问题，影响使用体验', 2, '电容耐温等级不足，长期高温环境下容量衰减', '更换为105°C耐温电容，增加出货前老化测试时间', 4, '已与客户达成补偿协议', 1),
('TEST-QMS-RMA-002',
 1, 568, '客户投诉AC插座插拔困难，部分产品插脚变形', 3, NULL, NULL, 2, '正在调查中', 1),
('TEST-QMS-RMA-003',
 7, 569, '客户反馈电源适配器外壳温度过高，超过60°C', 2, '散热设计不足，外壳材料导热系数偏高', '优化散热结构，更换低导热外壳材料', 3, '已提交改善方案', 1),
('TEST-QMS-RMA-004',
 3, 565, '小批量样品测试反馈亮度不均匀', 1, NULL, NULL, 1, '轻微问题，待进一步确认', 1);

COMMIT;
