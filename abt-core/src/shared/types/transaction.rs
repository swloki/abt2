/// 显式事务模式声明
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionMode {
    /// 调用者事务内执行（如库存预留、质量关卡）
    InCallerTx,
    /// 独立事务，调用者提交后执行（如 CostEntry 成本记录）
    IndependentTx,
    /// Outbox 异步消费（如 DocumentLink、Workflow 触发）
    AsyncOutbox,
}
