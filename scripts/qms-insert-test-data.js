const pg = require('pg');
async function run() {
  const c = new pg.Client('postgres://postgres:123456@127.0.0.1:5432/abt_v2');
  await c.connect();
  await c.query('BEGIN');
  await c.query("DELETE FROM rmas WHERE doc_number LIKE 'TEST-QMS-%'");
  await c.query("DELETE FROM mrbs WHERE doc_number LIKE 'TEST-QMS-%'");
  await c.query("DELETE FROM inspection_results WHERE doc_number LIKE 'TEST-QMS-%'");
  await c.query("DELETE FROM inspection_specifications WHERE doc_number LIKE 'TEST-QMS-%'");

  const specIds = {};
  const specs = [
    ['TEST-QMS-SPEC-001', 565, 1, [{item:'外观检查',standard:'无划痕'},{item:'电参数测试',standard:'正向电压3.2V'},{item:'光通量测试',standard:'50lm以上'},{item:'色温检测',standard:'6000-6500K'},{item:'焊接拉力',standard:'1.5N以上'}], {level:'II',aql:'1.0',mode:'Normal'}, 2, 1, 1],
    ['TEST-QMS-SPEC-002', 568, 1, [{item:'外观检查',standard:'无毛刺'},{item:'尺寸测量',standard:'符合图纸'},{item:'插拔力测试',standard:'3-8N'}], {level:'I',aql:'1.5',mode:'Normal'}, 2, 1, 1],
    ['TEST-QMS-SPEC-003', 569, 2, [{item:'锡膏厚度',standard:'0.12mm'},{item:'贴片偏移',standard:'0.1mm以内'},{item:'回流温度',standard:'标准曲线'},{item:'X-Ray',standard:'无虚焊'}], {level:'II',aql:'0.65',mode:'Normal'}, 2, 1, 1],
    ['TEST-QMS-SPEC-004', 565, 3, [{item:'电性能测试',standard:'100-240V'},{item:'耐压测试',standard:'3000V'},{item:'接地电阻',standard:'0.1Ohm以下'},{item:'老化测试',standard:'满载4h'},{item:'外观检查',standard:'无划痕'}], {level:'III',aql:'0.25',mode:'Tightened'}, 2, 1, 1],
    ['TEST-QMS-SPEC-005', 565, 4, [{item:'包装完整性',standard:'无破损'},{item:'标签核对',standard:'与订单一致'},{item:'数量清点',standard:'与装箱单一致'}], {level:'II',aql:'2.5',mode:'Normal'}, 2, 1, 1],
    ['TEST-QMS-SPEC-006', 569, 2, [{item:'电压测试',standard:'输出12V'}], {level:'II',aql:'1.0',mode:'Normal'}, 1, 1, 1],
    ['TEST-QMS-SPEC-007', 568, 4, [{item:'外观',standard:'无缺陷'}], {level:'I',aql:'2.5',mode:'Normal'}, 3, 1, 1],
    ['TEST-QMS-SPEC-008', 569, 3, [{item:'功率测试',standard:'400W'},{item:'温升测试',standard:'65C以下'},{item:'绝缘电阻',standard:'2MOhm以上'}], {level:'II',aql:'1.0',mode:'Normal'}, 2, 1, 1],
  ];
  for (const s of specs) {
    const r = await c.query(
      'INSERT INTO inspection_specifications (doc_number, product_id, inspection_type, check_items, sample_plan, status, version, operator_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id',
      [s[0],s[1],s[2],JSON.stringify(s[3]),JSON.stringify(s[4]),s[5],s[6],s[7]]
    );
    specIds[s[0]] = r.rows[0].id;
  }

  const resultIds = {};
  const results = [
    ['TEST-QMS-RES-001','TEST-QMS-SPEC-001',1,1,1,'B2026060801',50,50,0,1,[{item:'外观',measured:'合格',pass:true}],1,'2026-06-08',2],
    ['TEST-QMS-RES-002','TEST-QMS-SPEC-002',1,2,1,'B2026060802',40,36,4,2,[{item:'外观',measured:'4件毛刺',pass:false}],1,'2026-06-07',3],
    ['TEST-QMS-RES-003','TEST-QMS-SPEC-001',1,3,1,'B2026060803',80,78,2,3,[{item:'外观',measured:'合格',pass:true}],1,'2026-06-06',3],
    ['TEST-QMS-RES-004','TEST-QMS-SPEC-003',2,1,2,'B2026060601',30,30,0,1,[{item:'锡膏',measured:'0.13mm',pass:true}],1,'2026-06-06',2],
    ['TEST-QMS-RES-005','TEST-QMS-SPEC-003',2,2,2,'B2026060501',25,23,2,2,[{item:'偏移',measured:'0.3mm',pass:false}],1,'2026-06-05',2],
    ['TEST-QMS-RES-006','TEST-QMS-SPEC-004',2,3,3,'B2026060401',100,100,0,1,[{item:'电性能',measured:'合格',pass:true}],1,'2026-06-04',3],
    ['TEST-QMS-RES-007','TEST-QMS-SPEC-004',2,4,3,'B2026060301',60,55,5,2,[{item:'偏差',measured:'6%',pass:false}],1,'2026-06-03',2],
    ['TEST-QMS-RES-008','TEST-QMS-SPEC-005',3,1,4,'B2026060201',20,20,0,1,[{item:'包装',measured:'合格',pass:true}],1,'2026-06-02',3],
    ['TEST-QMS-RES-009','TEST-QMS-SPEC-005',3,2,4,'B2026060202',20,18,2,2,[{item:'包装',measured:'2箱破损',pass:false}],1,'2026-06-02',2],
    ['TEST-QMS-RES-010','TEST-QMS-SPEC-001',1,4,1,'B2026060901',50,0,0,1,[],0,null,1],
  ];
  for (const r of results) {
    const res = await c.query(
      'INSERT INTO inspection_results (doc_number, spec_id, source_type, source_id, inspection_type, batch_no, sample_qty, qualified_qty, unqualified_qty, result, check_results, inspector_id, inspection_date, status, operator_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) RETURNING id',
      [r[0],specIds[r[1]],r[2],r[3],r[4],r[5],r[6],r[7],r[8],r[9],JSON.stringify(r[10]),r[11],r[12],r[13],1]
    );
    resultIds[r[0]] = res.rows[0].id;
  }

  const mrbs = [
    ['TEST-QMS-MRB-001','TEST-QMS-RES-002',568,'来料检验发现AC插座4件毛刺',4,2,280,2,'供应商责任',1],
    ['TEST-QMS-MRB-002','TEST-QMS-RES-005',569,'SMT贴片偏移U1偏移0.3mm',4,1,860,1,'内部制程问题',1],
    ['TEST-QMS-MRB-003','TEST-QMS-RES-007',565,'输出电压偏差超出规格',2,2,5400,4,'已退货',1],
    ['TEST-QMS-MRB-004','TEST-QMS-RES-009',565,'包装破损外壳刮痕',3,1,2150,3,'降级处理',1],
    ['TEST-QMS-MRB-005','TEST-QMS-RES-005',569,'老化测试后外壳变色',1,1,1280,2,'报废',1],
  ];
  for (const m of mrbs) {
    await c.query(
      'INSERT INTO mrbs (doc_number, inspection_result_id, product_id, defect_description, disposition, responsible_party, cost_impact, status, remark, operator_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)',
      [m[0],resultIds[m[1]],m[2],m[3],m[4],m[5],m[6],m[7],m[8],m[9]]
    );
  }

  const rmas = [
    ['TEST-QMS-RMA-001',7,565,'客户反馈产品闪烁',2,'电容耐温不足','更换电容',4,'已补偿',1],
    ['TEST-QMS-RMA-002',1,568,'AC插座插拔困难',3,null,null,2,'调查中',1],
    ['TEST-QMS-RMA-003',7,569,'外壳温度过高',2,'散热设计不足','优化散热',3,'已提交方案',1],
    ['TEST-QMS-RMA-004',3,565,'样品亮度不均匀',1,null,null,1,'轻微问题',1],
  ];
  for (const r of rmas) {
    await c.query(
      'INSERT INTO rmas (doc_number, customer_id, product_id, defect_description, severity, root_cause, corrective_action, status, remark, operator_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)',
      [r[0],r[1],r[2],r[3],r[4],r[5],r[6],r[7],r[8],r[9]]
    );
  }

  await c.query('COMMIT');
  console.log('OK: specs=' + Object.keys(specIds).length + ' results=' + Object.keys(resultIds).length + ' mrbs=' + mrbs.length + ' rmas=' + rmas.length);
  await c.end();
}
run().catch(e => { console.error('ERR:', e.message); process.exit(1); });
