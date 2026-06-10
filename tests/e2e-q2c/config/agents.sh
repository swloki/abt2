#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — Agent 角色定义
# 定义 15 个测试角色，每个角色对应一个独立的浏览器会话
# ============================================================================

# 密码统一 test1234
Q2C_PASSWORD="test1234"
Q2C_HASH='$argon2id$v=19$m=19456,t=2,p=1$L8UrLwaarCfo9Rbt3WYUcw$Lce7XDcj10xhkEBv6TSBl/YtJ0bNsEy51HdE42tVKvI'

# --- Agent: 销售域 ---
# Agent-S1: 销售专员
AGENT_S1_USER="q2c_sales"
AGENT_S1_SESSION="q2c_sales"

# Agent-S2: 销售经理
AGENT_S2_USER="q2c_sales_mgr"
AGENT_S2_SESSION="q2c_sales_mgr"

# --- Agent: 计划域 ---
# Agent-P1: 计划员
AGENT_P1_USER="q2c_planner"
AGENT_P1_SESSION="q2c_planner"

# --- Agent: 采购域 ---
# Agent-PU1: 采购专员
AGENT_PU1_USER="q2c_buyer"
AGENT_PU1_SESSION="q2c_buyer"

# Agent-PU2: 采购经理
AGENT_PU2_USER="q2c_buyer_mgr"
AGENT_PU2_SESSION="q2c_buyer_mgr"

# --- Agent: 生产域 ---
# Agent-M1: 生产主管
AGENT_M1_USER="q2c_prod_mgr"
AGENT_M1_SESSION="q2c_prod_mgr"

# Agent-M2: 车间操作员
AGENT_M2_USER="q2c_operator"
AGENT_M2_SESSION="q2c_operator"

# --- Agent: 质量域 ---
# Agent-Q1: 质检员
AGENT_Q1_USER="q2c_qc"
AGENT_Q1_SESSION="q2c_qc"

# Agent-QM1: 质量主管
AGENT_QM1_USER="q2c_qc_mgr"
AGENT_QM1_SESSION="q2c_qc_mgr"

# --- Agent: 仓储域 ---
# Agent-W1: 仓管员
AGENT_W1_USER="q2c_warehouse"
AGENT_W1_SESSION="q2c_warehouse"

# --- Agent: 财务域 ---
# Agent-F1: 财务会计（应收/应付/发票/核销）
AGENT_F1_USER="q2c_accountant"
AGENT_F1_SESSION="q2c_accountant"

# Agent-F2: 成本会计
AGENT_F2_USER="q2c_cost_acct"
AGENT_F2_SESSION="q2c_cost_acct"

# Agent-F3: 出纳
AGENT_F3_USER="q2c_cashier"
AGENT_F3_SESSION="q2c_cashier"

# Agent-F4: 总账会计
AGENT_F4_USER="q2c_gl_acct"
AGENT_F4_SESSION="q2c_gl_acct"

# --- Agent: 管理层 ---
# Agent-GM: 总经理（会签审批）
AGENT_GM_USER="q2c_gm"
AGENT_GM_SESSION="q2c_gm"

# --- 所有 Agent 列表（用于批量操作） ---
ALL_AGENTS=(
    "S1:q2c_sales"
    "S2:q2c_sales_mgr"
    "P1:q2c_planner"
    "PU1:q2c_buyer"
    "PU2:q2c_buyer_mgr"
    "M1:q2c_prod_mgr"
    "M2:q2c_operator"
    "Q1:q2c_qc"
    "QM1:q2c_qc_mgr"
    "W1:q2c_warehouse"
    "F1:q2c_accountant"
    "F2:q2c_cost_acct"
    "F3:q2c_cashier"
    "F4:q2c_gl_acct"
    "GM:q2c_gm"
)

# --- 获取 Agent session 名 ---
# 用法: get_session <role_prefix>
# 例: get_session S1 → q2c_sales
get_session() {
    local role="$1"
    local var_name="AGENT_${role}_SESSION"
    echo "${!var_name}"
}

# --- 获取 Agent 用户名 ---
get_user() {
    local role="$1"
    local var_name="AGENT_${role}_USER"
    echo "${!var_name}"
}
