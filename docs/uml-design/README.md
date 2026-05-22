# UML 类图设计文档

> 2026-05-22 设计，基于 target.md 蓝图

## 文件说明

### HTML 可视化预览（Mermaid + 缩放/拖拽）

| 文件 | 内容 | 实体数 |
|------|------|--------|
| [00-module-dependencies.html](00-module-dependencies.html) | 模块间接口依赖关系总览 | 9 模块 + 48 Service trait |
| [00-shared-infrastructure.html](00-shared-infrastructure.html) | 共享基础设施层 — 文档编号、文档关联、库存预留、成本账本、领域事件(Outbox)、状态机、审计日志、幂等去重 | 8 核心 + 9 枚举 |
| [01-sales.html](01-sales.html) | 销售模块 — 报价、订单、发货、退货、对账 | 5 主表 + 5 明细表 |
| [02-purchase.html](02-purchase.html) | 采购模块 — 供应商、采购报价、订单、退货、对账、付款、零星请购 | 7 主表 + 6 明细表 |
| [03-wms.html](03-wms.html) | 仓储模块 — 三级库位、策略引擎、来料、库存事务、领料、倒冲、盘点、调拨、形态转换、锁库 | 12 主表 + 10 明细表 |
| [04-mes.html](04-mes.html) | 生产模块 — 计划、工单、工序、报工、报检、完工入库（委外委托 OM） | 6 主表 + 2 明细表 |
| [05-outsourcing.html](05-outsourcing.html) | 委外管理 — 委外单、发料明细、追踪节点、转自制 | 3 主表 + 7 节点类型 |
| [06-qms.html](06-qms.html) | 质量管理 — 检验规格、检验结果、MRB不良评审、RMA客诉 | 4 主表 + 9 枚举 |
| [07-fms.html](07-fms.html) | 财务管理 — 日记账、日记账明细、核销、费用报销、成本核算 | 5 主表 + 1 明细表 |
| [08-workflow-engine.html](08-workflow-engine.html) | 工作流引擎 V2 — 依赖共享层事件/状态机，Saga 补偿 + 增强节点 | 3 Service + 4 核心实体 |

## 查看方式

- **HTML**: 直接在浏览器中打开任意 `.html` 文件，支持滚轮缩放、拖拽平移

## 设计原则

- **接口先行**: 每个 Service trait 定义清晰的输入输出，模块间通过接口交互
- **共享层解耦**: DocumentSequence / DocumentLink / InventoryReservation / CostEntry 通过 DocumentType 枚举解耦
- **分层包结构**: Migration → Model → Repository → Service Trait → Service Impl → Handler → Proto
- **业财一体**: 从第一天起记录成本，避免事后对账
- **委外统一**: 委外供应商 = WMS 虚拟库位，复用已有调拨/入库模型
- **领域事件**: DomainEventBus 跨模块解耦，Outbox 模式 + 异步分发，LISTEN/NOTIFY 低延迟驱动
- **状态机**: StateMachineService 统一管理单据生命周期，转换规则存 DB 可配置
- **Saga 补偿**: 基于增强 WorkflowEngine 的长流程编排，支持失败逆序补偿
- **质量关卡**: 检验不合格自动阻断下游流转（数据层面硬门）
- **双层记账**: 每笔日记账同时记借方和贷方，支持成本中心和利润中心归集

## 接口统计

| 模块 | Service trait 数量 | 核心方法数 |
|------|-------------------|-----------|
| Shared | 9 | 20 |
| Sales CRM | 5 | 25 |
| Purchase SRM | 7 | 26 |
| WMS | 9 | 28 |
| MES | 5 | 19 |
| Outsourcing OM | 2 | 9 |
| Quality QMS | 4 | 16 |
| Financial FMS | 4 | 15 |
| Workflow V2 | 3 | 29 |
| **合计** | **48** | **187** |
