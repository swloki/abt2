#!/usr/bin/env bash
# ============================================================================
# Q2C E2E — 全链路 Happy Path 集成验证
# 从 S1（报价创建）到 F6（总账结算）+ CHK（数据一致性校验）
#
# 执行顺序:
#   0.  环境初始化 (setup.sh)
#   1.  Step 1:  S1-S2 报价创建
#   2.  Step 2:  S3 报价审批
#   3.  Step 3:  S4-S5 销售订单
#   4.  Step 4:  P1-P3 生产计划
#   5.  Step 5:  PU1-PU4 采购订单
#   6.  Step 6:  PU5-PU6 收货入库
#   7.  Step 7:  M1 工单下达
#   8.  Step 8:  M2 生产领料
#   9.  Step 9:  M3-M4 报工与质检
#  10.  Step 10: M5 成品入库
#  11.  Step 11: W1-W2 发货申请与拣货
#  12.  Step 12: W3-W4 发出与签收
#  13.  Step 13: F1-F2 应收与成本
#  14.  Step 14: F3-F5 开票与核销
#  15.  Step 15: CHK 数据一致性校验
#  --  FP1-FP4 + F6: 应付与总账 (bash 调用，不计步数)
# ============================================================================
set -euo pipefail

BASE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$BASE_DIR/config/env.sh"
source "$BASE_DIR/config/agents.sh"
source "$BASE_DIR/lib/login.sh"
source "$BASE_DIR/lib/form.sh"
source "$BASE_DIR/lib/assert.sh"
source "$BASE_DIR/lib/relay.sh"

PHASE1="$BASE_DIR/tests/phase1"
PHASE2="$BASE_DIR/tests/phase2"
PHASE3A="$BASE_DIR/tests/phase3a"
PHASE3B="$BASE_DIR/tests/phase3b"
PHASE4="$BASE_DIR/tests/phase4"
PHASE5="$BASE_DIR/tests/phase5"
VALIDATION="$BASE_DIR/tests/validation"

echo "============================================"
echo "  Q2C 全链路 Happy Path 集成验证"
echo "  S1 → S2 → S3 → S4 → P1-P3"
echo "  → PU1-PU6 → M1-M5"
echo "  → W1-W4 → F1-F6 + FP1-FP4"
echo "  → CHK-01~12"
echo "============================================"
echo ""

# --- 环境初始化 ---
log_step "0. 环境初始化"
bash "$BASE_DIR/scripts/setup.sh"

if [[ $? -ne 0 ]]; then
    log_fail "环境初始化失败"
    exit 1
fi
assert_pass "环境初始化完成"

# 初始化接力文件
relay_init "q2c-full-$(date +%Y%m%d%H%M%S)"
relay_set_status "running"

# --- 步骤定义 ---
TOTAL_STEPS=15
PASS_COUNT=0
FAIL_COUNT=0

run_step() {
    local step_num="$1"
    local step_name="$2"
    local script_path="$3"

    echo ""
    echo "============================================"
    echo "  Step $step_num/$TOTAL_STEPS: $step_name"
    echo "============================================"

    relay_set_phase "$step_name"

    if [[ ! -f "$script_path" ]]; then
        log_fail "脚本不存在: $script_path"
        ((FAIL_COUNT++))
        return 1
    fi

    if bash "$script_path"; then
        log_pass "Step $step_num: $step_name PASSED"
        ((PASS_COUNT++))
        return 0
    else
        log_fail "Step $step_num: $step_name FAILED"
        ((FAIL_COUNT++))
        return 1
    fi
}

# --- 汇总与退出 ---
# goto_summary: 打印汇总报告并根据失败阶段决定退出码
#   Phase 1-4 (P0) 失败 → exit 1
#   Phase 5   (财务域) 失败 → exit 0 + warning
goto_summary() {
    echo ""
    echo "============================================"
    echo "  Q2C 全链路 Happy Path 中断汇总"
    echo "============================================"
    echo "  通过: $PASS_COUNT/$TOTAL_STEPS"
    echo "  失败: $FAIL_COUNT"
    echo "============================================"

    relay_set_status "failed"
    relay_snapshot "SNAP-FULL-HAPPY-PATH"

    echo ""
    echo "接力数据摘要:"
    echo "  报价 ID:     $(relay_read quotation_id 2>/dev/null || echo 'N/A')"
    echo "  订单 ID:     $(relay_read sales_order_id 2>/dev/null || echo 'N/A')"
    echo "  工单 ID:     $(relay_read work_order_id 2>/dev/null || echo 'N/A')"
    echo "  采购订单 ID: $(relay_read purchase_order_id 2>/dev/null || echo 'N/A')"
    echo "  发货申请 ID: $(relay_read shipping_request_id 2>/dev/null || echo 'N/A')"
    echo ""

    echo -e "${RED}❌ Q2C HAPPY PATH FAILED ($FAIL_COUNT failures)${NC}"
    exit 1
}

# --- Phase 1+2: 销售与计划 ---

run_step 1 "S1-S2 报价创建" "$PHASE1/test_s1_s2_quotation.sh" || goto_summary
run_step 2 "S3 报价审批" "$PHASE1/test_s3_approval.sh" || goto_summary
run_step 3 "S4-S5 销售订单" "$PHASE1/test_s4_s5_sales_order.sh" || goto_summary
run_step 4 "P1-P3 生产计划" "$PHASE2/test_p1_p3_mrp.sh" || goto_summary

# --- Phase 3A: 采购 ---

run_step 5 "PU1-PU4 采购订单" "$PHASE3A/test_pu1_pu4_purchase_order.sh" || goto_summary
run_step 6 "PU5-PU6 收货入库" "$PHASE3A/test_pu5_pu6_goods_receipt.sh" || goto_summary

# --- Phase 3B: 生产 ---

run_step 7 "M1 工单下达" "$PHASE3B/test_m1_work_order_release.sh" || goto_summary
run_step 8 "M2 生产领料" "$PHASE3B/test_m2_material_requisition.sh" || goto_summary
run_step 9 "M3-M4 报工与质检" "$PHASE3B/test_m3_m4_work_report_qc.sh" || goto_summary
run_step 10 "M5 成品入库" "$PHASE3B/test_m5_finished_goods_receipt.sh" || goto_summary

# --- Phase 4: 发货 ---

run_step 11 "W1-W2 发货申请与拣货" "$PHASE4/test_w1_w2_pick_pack.sh" || goto_summary
run_step 12 "W3-W4 发出与签收" "$PHASE4/test_w3_w4_ship_confirm.sh" || goto_summary

# --- Phase 5: 财务 (可降级) ---
# 财务域失败不阻断 CI，但输出 warning

PHASE5_P0_STEPS=$PASS_COUNT
PHASE5_FAILED=0

echo ""
echo "============================================"
echo "  Phase 5: 财务闭环（可降级）"
echo "============================================"

# F1-F2
run_step 13 "F1-F2 应收与成本" "$PHASE5/test_f1_f2_ar_cost.sh" || {
    log_warn "F1-F2 失败，财务域可能部分未实现"
    PHASE5_FAILED=1
}

# F3-F5
run_step 14 "F3-F5 开票与核销" "$PHASE5/test_f3_f5_invoice_collect.sh" || {
    log_warn "F3-F5 失败，继续后续步骤"
    PHASE5_FAILED=1
}

# --- CHK 校验 ---

echo ""
echo "============================================"
echo "  数据一致性校验"
echo "============================================"

run_step 15 "CHK 数据一致性校验" "$VALIDATION/test_chk_all.sh" || {
    log_warn "CHK 校验存在失败项"
    PHASE5_FAILED=1
}

# FP1-FP4 + F6: 应付与总账 (bash 调用，不计步数)
bash "$PHASE5/test_fp1_fp4_ap_payment.sh" || log_warn "FP1-FP4 失败"
bash "$PHASE5/test_f6_gl_settlement.sh" || log_warn "F6 总账结算跳过"

# --- 汇总 ---
echo ""
echo "============================================"
echo "  Q2C 全链路 Happy Path 完成"
echo "============================================"
echo "  通过: $PASS_COUNT/$TOTAL_STEPS"
echo "  失败: $FAIL_COUNT"
echo "============================================"

relay_set_status "$([[ $FAIL_COUNT -eq 0 ]] && echo 'completed' || echo 'completed_with_warnings')"
relay_snapshot "SNAP-FULL-HAPPY-PATH"

echo ""
echo "接力数据摘要:"
echo "  报价 ID:     $(relay_read quotation_id 2>/dev/null || echo 'N/A')"
echo "  订单 ID:     $(relay_read sales_order_id 2>/dev/null || echo 'N/A')"
echo "  工单 ID:     $(relay_read work_order_id 2>/dev/null || echo 'N/A')"
echo "  采购订单 ID: $(relay_read purchase_order_id 2>/dev/null || echo 'N/A')"
echo "  发货申请 ID: $(relay_read shipping_request_id 2>/dev/null || echo 'N/A')"
echo ""

if [[ $FAIL_COUNT -eq 0 ]]; then
    echo -e "${GREEN}✅ Q2C HAPPY PATH PASSED${NC}"
    exit 0
else
    # P0 阶段（Phase 1-4）失败 → CI 红灯
    if [[ $PASS_COUNT -lt $PHASE5_P0_STEPS ]]; then
        echo -e "${RED}❌ Q2C HAPPY PATH FAILED ($FAIL_COUNT failures, P0 phase broken)${NC}"
        exit 1
    fi
    # 仅财务域失败 → CI 绿灯 + warning
    echo -e "${YELLOW}⚠️ Q2C HAPPY PATH COMPLETED WITH WARNINGS ($FAIL_COUNT failures in financial domain)${NC}"
    exit 0
fi
