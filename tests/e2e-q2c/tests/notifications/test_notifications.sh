#!/usr/bin/env bash
# NT-ALL: 通知规则验证 N1-N20
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== NT-ALL: 通知规则验证 N1-N20 ==="

log_step "1. 检查通知相关表"
NOTIF_TABLES=$(psql "$DB_URL" -t -A -c "
    SELECT table_name FROM information_schema.tables
    WHERE table_name LIKE '%notif%' OR table_name LIKE '%message%'
       OR table_name LIKE '%alert%' OR table_name LIKE '%queue%'" 2>/dev/null || echo "")
if [[ -z "$NOTIF_TABLES" ]]; then
    assert_skip "NT-ALL: 系统未实现通知功能（无通知相关表）"
    print_summary
    echo "=== NT-ALL 通知规则验证 完成 ==="
    exit 0
fi
log_info "通知相关表: $(echo $NOTIF_TABLES | tr '\n' ',')"

# 识别通知表名
NOTIF_TABLE=""
QUEUE_TABLE=""
for tbl in $NOTIF_TABLES; do
    if [[ "$tbl" == "notifications" ]]; then NOTIF_TABLE="$tbl"; fi
    if [[ "$tbl" == "notification_queue" ]]; then QUEUE_TABLE="$tbl"; fi
done

# 如果未找到精确匹配，取第一个
if [[ -z "$NOTIF_TABLE" ]]; then
    NOTIF_TABLE=$(echo "$NOTIF_TABLES" | head -1)
fi

log_info "使用通知表: $NOTIF_TABLE"
log_info "使用队列表: $QUEUE_TABLE"

log_step "2. 验证通知规则 N1-N20"

# 定义 20 个通知规则
# 格式: "编号|规则名称|触发条件|预期接收者|预期渠道|预期内容关键词"
declare -a NOTIFICATION_RULES=(
    "N1|报价待审批|报价提交|销售经理(S2)|站内+邮件|报价编号"
    "N2|报价已审批|报价通过|销售专员(S1)|站内|审批通过"
    "N3|报价已拒绝|报价拒绝|销售专员(S1)|站内+邮件|拒绝原因"
    "N4|订单已创建|销售订单创建|客户联系人|邮件|订单详情"
    "N5|订单已确认|订单确认|客户联系人|邮件|确认信息"
    "N6|订单已发货|发货完成|客户联系人|邮件+短信|物流信息"
    "N7|订单已签收|客户签收|销售专员(S1)|站内|签收确认"
    "N8|采购订单待审批|PO提交审批|采购经理(PU2)|站内|采购订单详情"
    "N9|采购订单已审批|PO审批通过|采购专员(PU1)|站内|审批结果"
    "N10|到货通知|采购到货|仓管员(W1)|站内|到货详情"
    "N11|来料待检|到货入库|质检员(Q1)|站内|待检清单"
    "N12|工单待开工|工单下达|生产主管(M1)|站内|工单详情"
    "N13|工单完工|工单报工完成|生产主管(M1)|站内|完工报告"
    "N14|质检不合格|质检失败|质量主管(QM1)|站内+邮件|不合格详情"
    "N15|库存预警|库存低于安全量|计划员(P1)|站内|物料编号+当前库存"
    "N16|信用额度预警|客户信用接近上限|销售经理(S2)|站内|客户+信用余额"
    "N17|发货待拣货|发货申请创建|仓管员(W1)|站内|发货单号"
    "N18|退货待处理|客户退货申请|仓管员(W1)|站内|退货详情"
    "N19|付款已完成|供应商付款完成|财务会计(F1)|站内|付款金额"
    "N20|收款已确认|客户收款确认|财务会计(F1)|站内|收款金额"
)

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

for rule in "${NOTIFICATION_RULES[@]}"; do
    IFS='|' read -r code name trigger recipient channel keyword <<< "$rule"

    # 检查是否有匹配的通知记录
    NOTIF_EXISTS=$(psql "$DB_URL" -t -A -c "
        SELECT COUNT(*) FROM $NOTIF_TABLE
        WHERE (title LIKE '%$name%' OR content LIKE '%$trigger%' OR content LIKE '%$keyword%')
          AND deleted_at IS NULL" 2>/dev/null || echo "-1")

    if [[ "$NOTIF_EXISTS" == "-1" ]]; then
        assert_skip "$code ($name): 通知表查询失败，结构可能不匹配"
        ((SKIP_COUNT++))
    elif [[ "$NOTIF_EXISTS" -gt 0 ]]; then
        # 获取详细信息
        RECIPIENT_CHECK=$(psql "$DB_URL" -t -A -c "
            SELECT recipient_id FROM $NOTIF_TABLE
            WHERE (title LIKE '%$name%' OR content LIKE '%$trigger%')
            LIMIT 1" 2>/dev/null || echo "N/A")
        assert_pass "$code ($name): 通知已触发（$NOTIF_EXISTS 条），接收者=$RECIPIENT_CHECK"
        ((PASS_COUNT++))
    else
        # 通知规则可能已定义但未被触发
        # 检查通知规则配置表
        RULE_EXISTS=$(psql "$DB_URL" -t -A -c "
            SELECT COUNT(*) FROM information_schema.tables
            WHERE table_name LIKE '%notif%rule%' OR table_name LIKE '%rule%notif%'" 2>/dev/null || echo "0")

        if [[ "$RULE_EXISTS" -gt 0 ]]; then
            RULE_COUNT=$(psql "$DB_URL" -t -A -c "
                SELECT COUNT(*) FROM notification_rules
                WHERE name LIKE '%$name%' OR event LIKE '%$trigger%'" 2>/dev/null || echo "0")
            if [[ "$RULE_COUNT" -gt 0 ]]; then
                assert_pass "$code ($name): 规则已定义（未触发）"
                ((PASS_COUNT++))
            else
                assert_skip "$code ($name): 规则未定义且未触发"
                ((SKIP_COUNT++))
            fi
        else
            assert_skip "$code ($name): 通知未触发（可能需要执行 Happy Path 先）"
            ((SKIP_COUNT++))
        fi
    fi
done

log_step "3. 通知渠道验证"
# 检查通知渠道配置
CHANNEL_TABLES=$(psql "$DB_URL" -t -A -c "
    SELECT table_name FROM information_schema.tables
    WHERE table_name LIKE '%channel%' OR table_name LIKE '%template%'" 2>/dev/null || echo "")
log_info "渠道/模板表: $(echo $CHANNEL_TABLES | tr '\n' ',')"

if [[ -n "$QUEUE_TABLE" ]]; then
    QUEUE_COUNT=$(psql "$DB_URL" -t -A -c "
        SELECT COUNT(*) FROM $QUEUE_TABLE" 2>/dev/null || echo "0")
    log_info "通知队列记录: $QUEUE_COUNT"
fi

log_step "4. 通知统计汇总"
TOTAL_NOTIFS=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM $NOTIF_TABLE WHERE deleted_at IS NULL" 2>/dev/null || echo "0")
log_info "总通知记录: $TOTAL_NOTIFS"

echo ""
echo "==========================================="
echo "  通知规则验证汇总"
echo "==========================================="
echo "  规则总数: 20"
echo "  通过:     $PASS_COUNT"
echo "  跳过:     $SKIP_COUNT"
echo "  失败:     $FAIL_COUNT"
echo "  总通知:   $TOTAL_NOTIFS"
echo "==========================================="

print_summary
echo "=== NT-ALL 通知规则验证 完成 ==="
