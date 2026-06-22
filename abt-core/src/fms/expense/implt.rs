use chrono::Datelike;
use sqlx::PgPool;
use rust_decimal::Decimal;

use crate::fms::cash_journal::repo::{CashJournalLineRepo, CashJournalRepo};
use crate::fms::enums::{
    CashDirection, ExpenseStatus, JournalStatus, JournalType,
};
use crate::fms::expense::model::*;
use crate::fms::expense::repo::{ExpenseAttachmentRepo, ExpenseReimbursementItemRepo, ExpenseReimbursementRepo};
use crate::gl::entry::{model::GlEntryLineInput, new_gl_entry_service, service::GlEntryService};
use crate::gl::mapping::{new_gl_mapping_service, service::GlMappingService};
use crate::fms::expense::service::ExpenseReimbursementService;
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService, RecordAuditLogReq};
use crate::shared::document_sequence::service::DocumentSequenceService;
use crate::shared::document_sequence::new_document_sequence_service;
use crate::shared::enums::audit::AuditAction;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::enums::event::DomainEventType;
use crate::shared::event_bus::model::EventPublishRequest;
use crate::shared::event_bus::service::DomainEventBus;
use crate::shared::event_bus::new_domain_event_bus;
use crate::shared::identity::user_service::UserService;
use crate::shared::notification::model::{BatchNotificationReq, NotificationType};
use crate::shared::notification::service::NotificationService;
use crate::shared::notification::new_notification_service;
use crate::shared::state_machine::service::StateMachineService;
use crate::shared::state_machine::new_state_machine_service;
use crate::shared::types::context::ServiceContext;
use crate::shared::types::{DomainError, PageParams, PaginatedResult, PgExecutor, Result};

pub struct ExpenseReimbursementServiceImpl {
    pool: PgPool,
}

impl ExpenseReimbursementServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 报销付款过账：借 default_expense / 贷 default_bank
    async fn post_expense_gl(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        expense: &ExpenseReimbursement,
    ) -> Result<()> {
        let map = new_gl_mapping_service(self.pool.clone());
        let expense_acct = map.resolve(ctx, db, "default_expense", None).await?;
        let bank_acct = map.resolve(ctx, db, "default_bank", None).await?;

        let items = ExpenseReimbursementItemRepo::get_by_reimbursement_id(db, expense.id).await?;
        let (cc, pc) = items
            .first()
            .map(|i| (i.cost_center, i.profit_center))
            .unwrap_or((None, None));

        let amt = expense.total_amount;
        let gl_lines = vec![
            GlEntryLineInput {
                account_id: expense_acct,
                debit: amt,
                credit: Decimal::ZERO,
                cost_center: cc,
                profit_center: pc,
                project_id: None,
                memo: format!("报销付款 {}", expense.doc_number),
            },
            GlEntryLineInput {
                account_id: bank_acct,
                debit: Decimal::ZERO,
                credit: amt,
                cost_center: cc,
                profit_center: pc,
                project_id: None,
                memo: format!("报销付款 {}", expense.doc_number),
            },
        ];

        new_gl_entry_service(self.pool.clone())
            .post_from_source(
                ctx,
                db,
                DocumentType::ExpenseReimbursement,
                expense.id,
                expense.expense_date,
                format!("报销付款 {}", expense.doc_number),
                gl_lines,
            )
            .await?;
        Ok(())
    }

    /// 发送通知给单个用户
    async fn notify_user(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        user_id: i64,
        title: &str,
        content: &str,
        related_id: i64,
    ) {
        let req = BatchNotificationReq {
            notification_type: NotificationType::Business,
            title: title.to_string(),
            content: Some(content.to_string()),
            related_type: Some("ExpenseReimbursement".to_string()),
            related_id: Some(related_id),
        };
        let _ = new_notification_service(self.pool.clone())
            .batch_create_notifications(ctx, db, &[user_id], req)
            .await;
    }

    /// 发送通知给角色下所有用户（通过 role_code 查询 role_id）
    async fn notify_role_by_code(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        role_code: &str,
        title: &str,
        content: &str,
        related_id: i64,
    ) {
        let role_id: Option<i64> = sqlx::query_scalar(
            "SELECT role_id FROM roles WHERE role_code = $1",
        )
        .bind(role_code)
        .fetch_optional(&mut *db)
        .await
        .ok()
        .flatten();

        if let Some(rid) = role_id {
            let req = BatchNotificationReq {
                notification_type: NotificationType::Business,
                title: title.to_string(),
                content: Some(content.to_string()),
                related_type: Some("ExpenseReimbursement".to_string()),
                related_id: Some(related_id),
            };
            let _ = new_notification_service(self.pool.clone())
                .notify_by_role(ctx, db, rid, req)
                .await;
        }
    }

    /// 构建审批进度
    async fn build_approval_progress(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        expense: &ExpenseReimbursement,
    ) -> Result<Vec<ApprovalProgressNode>> {
        let sm = new_state_machine_service(self.pool.clone());
        let history = sm
            .get_state_history(ctx, db, "ExpenseStatus", expense.id, 1, 50)
            .await?;

        // 根据当前状态判断哪些 stage 已完成
        let mut completed_stages: Vec<&str> = Vec::new();
        let current_stage = match expense.status {
            ExpenseStatus::Draft => None,
            ExpenseStatus::Submitted => {
                completed_stages.push("submit");
                Some("supervisor")
            }
            ExpenseStatus::SupervisorApproved => {
                completed_stages.extend_from_slice(&["submit", "supervisor"]);
                Some("finance")
            }
            ExpenseStatus::FinanceApproved => {
                completed_stages.extend_from_slice(&["submit", "supervisor", "finance"]);
                Some("gm")
            }
            ExpenseStatus::Approved => {
                completed_stages.extend_from_slice(&["submit", "supervisor", "finance", "gm"]);
                Some("cashier")
            }
            ExpenseStatus::Paid => {
                completed_stages.extend_from_slice(&["submit", "supervisor", "finance", "gm", "cashier"]);
                None
            }
            ExpenseStatus::Cancelled => None,
        };

        // 从 state history 中获取每个完成节点的操作人信息
        let mut operator_map: std::collections::HashMap<String, (i64, String)> =
            std::collections::HashMap::new();
        let mut operator_ids: Vec<i64> = Vec::new();

        for log in &history.items {
            let mapped_stage = match log.to_state.as_str() {
                "Submitted" => "submit",
                "SupervisorApproved" => "supervisor",
                "FinanceApproved" => "finance",
                "Approved" => "gm",
                "Paid" => "cashier",
                _ => continue,
            };
            operator_map.insert(
                mapped_stage.to_string(),
                (
                    log.operator_id,
                    log.created_at.format("%Y-%m-%d %H:%M").to_string(),
                ),
            );
            operator_ids.push(log.operator_id);
        }

        // 批量查询操作人名称
        let user_svc = crate::shared::identity::new_user_service(self.pool.clone());
        let user_names: std::collections::HashMap<i64, String> = if !operator_ids.is_empty() {
            user_svc
                .get_users_by_ids(ctx, db, operator_ids)
                .await
                .map(|users| {
                    users
                        .into_iter()
                        .map(|u| (u.user.user_id, u.user.display_name.unwrap_or(u.user.username)))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        };

        // 构建节点列表
        let stages = ["submit", "supervisor", "finance", "gm", "cashier"];
        let labels = ["提交报销", "直属上级审批", "财务审核", "总经理审批", "出纳付款"];

        let mut nodes = Vec::new();
        for (i, stage_name) in stages.iter().enumerate() {
            let stage = *stage_name; // &&str → &str
            let (status_str, op_name, op_at) = if Some(stage) == current_stage {
                let (n, t) = operator_map
                    .get(stage)
                    .map(|(id, ts)| (user_names.get(id).cloned(), Some(ts.clone())))
                    .unwrap_or((None, None));
                ("current", n, t)
            } else if completed_stages.contains(&stage) {
                let (n, t) = operator_map
                    .get(stage)
                    .map(|(id, ts)| (user_names.get(id).cloned(), Some(ts.clone())))
                    .unwrap_or((None, None));
                ("completed", n, t)
            } else {
                ("pending", None, None)
            };

            nodes.push(ApprovalProgressNode {
                stage: stage_name.to_string(),
                label: labels[i].to_string(),
                status: status_str.to_string(),
                operator_name: op_name,
                operated_at: op_at,
                remark: None,
            });
        }

        Ok(nodes)
    }

    /// IndependentTx — 保存付款信息 + 生成付款日记账 + 更新状态为 Paid（由 pay() 调用）
    pub async fn generate_payment_journal_with_info(
        &self,
        ctx: &ServiceContext,
        _db: PgExecutor<'_>,
        expense_id: i64,
        pay_req: &PayReq,
    ) -> Result<i64> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        let expense = ExpenseReimbursementRepo::get_by_id(&mut *tx, expense_id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::Approved {
            return Err(DomainError::business_rule(
                "Expense must be Approved before payment",
            ));
        }

        // Save payment info inside the transaction (atomic with journal creation)
        ExpenseReimbursementRepo::update_payment_info(
            &mut *tx,
            expense_id,
            &pay_req.payment_bank,
            &pay_req.payment_remark,
            pay_req.payment_date,
        )
        .await?;

        let cj_doc_number = {
            let cj_ctx = ServiceContext::new(expense.operator_id);
            new_document_sequence_service(self.pool.clone())
                .next_number(&cj_ctx, &mut *tx, DocumentType::CashJournal)
                .await?
        };

        let now = chrono::Utc::now();
        let journal_req = crate::fms::cash_journal::model::CreateCashJournalReq {
            journal_type: JournalType::Expense,
            direction: CashDirection::Outflow,
            amount: expense.total_amount,
            counterparty: crate::fms::enums::CounterpartyRef::Employee(expense.applicant_id),
            source_type: DocumentType::ExpenseReimbursement,
            source_id: expense.id,
            bank_account: expense.payment_bank.clone().unwrap_or_default(),
            transaction_date: now.date_naive(),
            period: format!("{}-{:02}", now.year(), now.month()),
            remark: expense
                .payment_remark
                .clone()
                .unwrap_or_else(|| expense.remark.clone()),
            lines: vec![
                crate::fms::cash_journal::model::CashJournalLineInput {
                    account_code: "应付职工薪酬".to_string(),
                    debit_amount: expense.total_amount,
                    credit_amount: rust_decimal::Decimal::ZERO,
                    cost_center: None,
                    profit_center: None,
                    remark: "报销付款 — 借方".to_string(),
                },
                crate::fms::cash_journal::model::CashJournalLineInput {
                    account_code: "银行存款".to_string(),
                    debit_amount: rust_decimal::Decimal::ZERO,
                    credit_amount: expense.total_amount,
                    cost_center: None,
                    profit_center: None,
                    remark: "报销付款 — 贷方".to_string(),
                },
            ],
        };

        let total_debit: rust_decimal::Decimal =
            journal_req.lines.iter().map(|l| l.debit_amount).sum();
        let total_credit: rust_decimal::Decimal =
            journal_req.lines.iter().map(|l| l.credit_amount).sum();
        if total_debit != total_credit {
            return Err(DomainError::business_rule("UnbalancedEntry"));
        }
        if total_debit == rust_decimal::Decimal::ZERO {
            return Err(DomainError::business_rule("ZeroEntry"));
        }

        let journal_id = CashJournalRepo::create(
            &mut *tx,
            &cj_doc_number,
            &journal_req,
            expense.operator_id,
        )
        .await?;

        CashJournalLineRepo::batch_insert(&mut *tx, journal_id, &journal_req.lines).await?;

        let journal_rows = CashJournalRepo::update_status(
            &mut *tx,
            journal_id,
            JournalStatus::Confirmed,
            1,
        )
        .await?;
        if journal_rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        let rows = ExpenseReimbursementRepo::update_status(
            &mut *tx,
            expense.id,
            ExpenseStatus::Paid,
            expense.version,
        )
        .await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        self.post_expense_gl(ctx, &mut *tx, &expense).await?;

        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;

        if let Ok(mut event_conn) = self.pool.acquire().await {
            let event_ctx = ServiceContext::new(expense.operator_id);
            new_domain_event_bus(self.pool.clone())
                .publish(
                    &event_ctx,
                    &mut event_conn,
                    EventPublishRequest {
                        event_type: DomainEventType::ExpensePaymentGenerated,
                        aggregate_type: "ExpenseReimbursement".to_string(),
                        aggregate_id: expense.id,
                        payload: serde_json::json!({
                            "expense_id": expense.id,
                            "doc_number": expense.doc_number,
                            "total_amount": expense.total_amount,
                            "cash_journal_id": journal_id,
                            "applicant_id": expense.applicant_id,
                        }),
                        idempotency_key: Some(format!("ExpensePayment:{}", expense.id)),
                    },
                )
                .await
                .ok();
        }

        Ok(journal_id)
    }
}

// ── Trait implementation ──

#[async_trait::async_trait]
impl ExpenseReimbursementService for ExpenseReimbursementServiceImpl {
    async fn create(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateExpenseReq,
    ) -> Result<i64> {
        if req.items.is_empty() {
            return Err(DomainError::validation("at least one expense item is required"));
        }

        for item in &req.items {
            if item.amount <= rust_decimal::Decimal::ZERO {
                return Err(DomainError::validation(
                    "expense item amount must be greater than zero",
                ));
            }
        }

        let total_amount: rust_decimal::Decimal = req.items.iter().map(|i| i.amount).sum();

        if total_amount <= rust_decimal::Decimal::ZERO {
            return Err(DomainError::validation("total amount must be greater than zero"));
        }

        let doc_number = new_document_sequence_service(self.pool.clone())
            .next_number(ctx, db, DocumentType::ExpenseReimbursement)
            .await?;

        let id = ExpenseReimbursementRepo::create(db, &doc_number, &req, total_amount, ctx.operator_id).await?;
        ExpenseReimbursementItemRepo::batch_insert(db, id, &req.items).await?;

        if !req.attachments.is_empty() {
            ExpenseAttachmentRepo::batch_insert(db, id, &req.attachments).await?;
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "Draft", None)
            .await?;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Create,
                    changes: None,
                    context: None,
                },
            )
            .await?;

        Ok(id)
    }

    async fn submit(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::Draft {
            return Err(DomainError::business_rule("Only Draft expenses can be submitted"));
        }

        // Auto-fetch direct supervisor from department leader
        if let Some(dept_id) = expense.department_id {
            if let Some(leader_id) = ExpenseReimbursementRepo::get_department_leader(db, dept_id).await? {
                ExpenseReimbursementRepo::update_supervisor(db, id, leader_id).await?;
            }
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "Submitted", None)
            .await?;

        let rows = ExpenseReimbursementRepo::update_status(db, id, ExpenseStatus::Submitted, expense.version).await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        // Notify supervisor — use the supervisor_id we just set, or re-fetch from department
        let supervisor_id = if let Some(dept_id) = expense.department_id {
            ExpenseReimbursementRepo::get_department_leader(db, dept_id).await?
        } else {
            None
        };
        if let Some(sid) = supervisor_id {
            self.notify_user(
                ctx,
                db,
                sid,
                "新的报销审批",
                &format!(
                    "{} 提交了报销申请 {}，金额 ¥{:.2}，请审批",
                    ctx.operator_id, expense.doc_number, expense.total_amount
                ),
                expense.id,
            )
            .await;
        }

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({ "from": "Draft", "to": "Submitted" })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn supervisor_approve(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: SupervisorApproveReq,
    ) -> Result<()> {
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::Submitted {
            return Err(DomainError::business_rule(
                "Only Submitted expenses can be supervisor-approved",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "SupervisorApproved", req.remark.as_deref())
            .await?;

        let rows = ExpenseReimbursementRepo::update_status(
            db,
            id,
            ExpenseStatus::SupervisorApproved,
            expense.version,
        )
        .await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        self.notify_role_by_code(
            ctx,
            db,
            "finance",
            "报销单待财务审核",
            &format!(
                "报销单 {}（金额 ¥{:.2}）已通过直属上级审批，请财务审核",
                expense.doc_number, expense.total_amount
            ),
            expense.id,
        )
        .await;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({
                        "from": "Submitted",
                        "to": "SupervisorApproved",
                        "remark": req.remark,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn finance_approve(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: FinanceApproveReq,
    ) -> Result<()> {
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::SupervisorApproved {
            return Err(DomainError::business_rule(
                "Only SupervisorApproved expenses can be finance-approved",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "FinanceApproved", req.remark.as_deref())
            .await?;

        let rows = ExpenseReimbursementRepo::update_status(
            db,
            id,
            ExpenseStatus::FinanceApproved,
            expense.version,
        )
        .await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        self.notify_role_by_code(
            ctx,
            db,
            "gm",
            "报销单待总经理审批",
            &format!(
                "报销单 {}（金额 ¥{:.2}）已通过财务审核，请总经理审批",
                expense.doc_number, expense.total_amount
            ),
            expense.id,
        )
        .await;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({
                        "from": "SupervisorApproved",
                        "to": "FinanceApproved",
                        "remark": req.remark,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn approve(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        // Now approve = GM approval, requires FinanceApproved status
        if expense.status != ExpenseStatus::FinanceApproved {
            return Err(DomainError::business_rule(
                "Only FinanceApproved expenses can be approved by GM",
            ));
        }

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "Approved", None)
            .await?;

        let rows = ExpenseReimbursementRepo::update_status(db, id, ExpenseStatus::Approved, expense.version).await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        self.notify_role_by_code(
            ctx,
            db,
            "cashier",
            "报销单待付款",
            &format!(
                "报销单 {}（金额 ¥{:.2}）已通过总经理审批，请安排付款",
                expense.doc_number, expense.total_amount
            ),
            expense.id,
        )
        .await;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({ "from": "FinanceApproved", "to": "Approved" })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn pay(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
        req: PayReq,
    ) -> Result<()> {
        // Guard: expense must be Approved
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status != ExpenseStatus::Approved {
            return Err(DomainError::business_rule("Only Approved expenses can be paid"));
        }

        // Persist payment info + generate journal in one transaction
        let payment_bank = req.payment_bank.clone();
        self.generate_payment_journal_with_info(ctx, db, id, &req).await?;

        // Notify applicant
        self.notify_user(
            ctx,
            db,
            expense.applicant_id,
            "报销已付款",
            &format!(
                "您的报销单 {}（金额 ¥{:.2}）已付款，银行：{}",
                expense.doc_number, expense.total_amount, payment_bank
            ),
            expense.id,
        )
        .await;

        // Audit log
        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({
                        "from": "Approved",
                        "to": "Paid",
                        "payment_bank": payment_bank,
                        "payment_remark": req.payment_remark,
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn cancel(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<()> {
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        if expense.status == ExpenseStatus::Paid || expense.status == ExpenseStatus::Cancelled {
            return Err(DomainError::business_rule(
                "Cannot cancel a Paid or already Cancelled expense",
            ));
        }

        let old_status_str = expense.status.as_str().to_string();

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, "ExpenseStatus", id, "Cancelled", None)
            .await?;

        let rows = ExpenseReimbursementRepo::update_status(db, id, ExpenseStatus::Cancelled, expense.version).await?;
        if rows == 0 {
            return Err(DomainError::ConcurrentConflict);
        }

        self.notify_user(
            ctx,
            db,
            expense.applicant_id,
            "报销已取消",
            &format!("您的报销单 {} 已被取消", expense.doc_number),
            expense.id,
        )
        .await;

        new_audit_log_service(self.pool.clone())
            .record(
                ctx,
                db,
                RecordAuditLogReq {
                    entity_type: "ExpenseReimbursement",
                    entity_id: id,
                    action: AuditAction::Transition,
                    changes: Some(serde_json::json!({
                        "from": old_status_str,
                        "to": "Cancelled"
                    })),
                    context: None,
                },
            )
            .await?;

        Ok(())
    }

    async fn get(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<ExpenseReimbursement> {
        ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))
    }

    async fn list(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        filter: ExpenseFilter,
        page: PageParams,
    ) -> Result<PaginatedResult<ExpenseReimbursement>> {
        let (items, total) = ExpenseReimbursementRepo::query(
            db,
            &filter,
            &page,
            ctx.data_scope,
            ctx.operator_id,
            ctx.department_id,
        )
        .await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    async fn list_items(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        reimbursement_id: i64,
    ) -> Result<Vec<ExpenseReimbursementItem>> {
        ExpenseReimbursementItemRepo::get_by_reimbursement_id(db, reimbursement_id).await
    }

    async fn get_approval_progress(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        id: i64,
    ) -> Result<Vec<ApprovalProgressNode>> {
        let expense = ExpenseReimbursementRepo::get_by_id(db, id)
            .await?
            .ok_or_else(|| DomainError::not_found("ExpenseReimbursement"))?;

        self.build_approval_progress(ctx, db, &expense).await
    }

    // ── Attachments ──

    async fn list_attachments(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        expense_id: i64,
    ) -> Result<Vec<ExpenseAttachment>> {
        ExpenseAttachmentRepo::list_by_expense_id(db, expense_id).await
    }

    async fn upload_attachment(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        expense_id: i64,
        req: CreateAttachmentReq,
    ) -> Result<i64> {
        ExpenseAttachmentRepo::insert(db, expense_id, &req).await
    }

    async fn delete_attachment(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
        attachment_id: i64,
    ) -> Result<()> {
        let rows = ExpenseAttachmentRepo::delete(db, attachment_id).await?;
        if rows == 0 {
            return Err(DomainError::not_found("ExpenseAttachment"));
        }
        Ok(())
    }

    async fn pending_summary(
        &self,
        _ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<(i64, Decimal)> {
        ExpenseReimbursementRepo::pending_summary(db).await
    }
}
