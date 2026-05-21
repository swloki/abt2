# ABT ERP 四模块 UML 类图设计规范

> 日期：2026-05-22
> 状态：已评审通过
> 基于：target.md 蓝图 + 现有 PDM/BOM 基础

## 1. 设计概览

本次设计覆盖 4 个业务模块 + 1 个共享基础设施层，共 **35 个实体** + **30+ 枚举**。

### 模块清单

| 模块 | 前缀 | 核心实体 |
|------|------|----------|
| 共享基础设施 | — | DocumentSequence, DocumentLink, InventoryReservation, CostEntry |
| 销售 CRM | QUO/SO/SR/SRT/REC | Quotation, SalesOrder, ShippingRequest, SalesReturn, Reconciliation |
| 采购 SRM | PQ/PO/PRT/PAY/MISC | Supplier, PurchaseQuotation, PurchaseOrder, PurchaseReturn, PurchaseReconciliation, PaymentRequest, MiscellaneousRequest |
| 仓储 WMS | AN/MR/BF/CC/TRF/FC/LCK | Warehouse, Zone, Bin, StockLedger, ArrivalNotice, InventoryTransaction, MaterialRequisition, BackflushRecord, CycleCount, InventoryTransfer, FormConversion, InventoryLock, PutawayStrategy, PickStrategy |
| 生产 MES | PP/WO/WR/OO/PI/PR | ProductionPlan, WorkOrder, WorkOrderRouting, WorkReport, OutsourcingOrder, ProductionInspection, ProductionReceipt |

### 设计原则

- **共享层先行**：所有模块消费同一组共享服务（编号、关联、预留、成本）
- **DocumentType 枚举解耦**：共享实体通过枚举而非外键关联具体模块
- **业财一体**：从第一天起通过 CostEntry 记录成本
- **委外统一**：委外 = WMS 虚拟库位，复用调拨/入库模型
- **主从表模式**：每个业务单据由 Header（主表）+ Item（明细行）组成

---

## 2. 共享基础设施层

### 2.1 DocumentSequence — 统一文档编号

```
entity DocumentSequence {
  id: UUID PK
  prefix: String              // "SO", "PO", "WO"
  current_value: i32
  date: Date                  // 按日期分段
  padding_length: i32         // 补零位数
}
// 生成格式: {prefix}-{yyyy}-{MM}-{seq}
// 例: SO-2026-05-00142

enum DocumentType {
  QUOTATION, SALES_ORDER, SHIPPING_REQUEST,
  SALES_RETURN, RECONCILIATION,
  PURCHASE_QUOTATION, PURCHASE_ORDER,
  PURCHASE_RETURN, MISCELLANEOUS_REQUEST,
  WORK_ORDER, OUTSOURCING_ORDER,
  PRODUCTION_PLAN, WORK_REPORT,
  PRODUCTION_INSPECTION, PRODUCTION_RECEIPT,
  INSPECTION_REPORT, MRB_REPORT,
  ARRIVAL_NOTICE, MATERIAL_REQUISITION,
  BACKFLUSH, CYCLE_COUNT,
  INVENTORY_TRANSFER, FORM_CONVERSION,
  INVENTORY_LOCK,
  RECEIPT, PAYMENT_REQUEST, INVOICE
}
```

**功能：** 为所有业务单据生成唯一、可读、有序的编号。
**解决：** 杜绝各模块自行编号导致的冲突、格式混乱、无法追溯。

### 2.2 DocumentLink — 文档关联图谱

```
entity DocumentLink {
  id: UUID PK
  source_type: DocumentType
  source_id: UUID
  target_type: DocumentType
  target_id: UUID
  link_type: LinkType
  created_at: Timestamp
  created_by: UUID FK→users
}

enum LinkType {
  DERIVED_FROM    // 报价→订单
  TRIGGERS        // 订单→发货申请
  REFERENCES      // 退货→原发货
  RECONCILES      // 对账单→发货
  INSPECTS        // 检验→来料通知
  FULFILLS        // 采购入库→采购单
  ALLOCATES       // 领料→工单
}
```

**功能：** 有向图记录任意两个业务单据间的关系，支持图遍历查询。
**解决：** 九大模块间单据关系零散记录，统一图谱后：一键追溯、BI 报表、合规审计。

### 2.3 InventoryReservation — 库存预留层

```
entity InventoryReservation {
  id: UUID PK
  product_id: UUID FK→products
  warehouse_id: UUID FK→warehouses
  reserved_qty: Decimal(10,6)
  reservation_type: ReservationType
  source_type: DocumentType
  source_id: UUID
  status: ReservationStatus
  priority: i32
  expires_at: Timestamp        // TTL自动过期
  created_at: Timestamp
}

enum ReservationType {
  HARD          // 工单锁定，不可抢占
  SOFT          // 销售预留，可被高优先级抢占
  SAFETY_STOCK  // 安全库存永久预留
}

enum ReservationStatus {
  ACTIVE, FULFILLED, CANCELLED, EXPIRED
}
```

**公式：** 可用量 = 现有量 − 已预留量
**解决：** 区分现有量和可用量（ATP），销售接单、采购建议、工单领料都需要。

### 2.4 CostEntry — 成本累积账本

```
entity CostEntry {
  id: UUID PK
  entity_type: CostEntityType
  entity_id: UUID
  cost_type: CostType
  debit_amount: Decimal(10,6)
  credit_amount: Decimal(10,6)
  cost_center: UUID             // 部门/产线
  profit_center: UUID
  period: String                // "2026-05"
  source_type: DocumentType
  source_id: UUID
  created_at: Timestamp
}

enum CostType {
  MATERIAL, LABOR, OVERHEAD,
  OUTSOURCE, REWORK, SCRAP
}

enum CostEntityType {
  PRODUCT, WORK_ORDER, SALES_ORDER,
  PURCHASE_ORDER, INSPECTION
}
```

**原则：** 双层记账——每个操作同时记借方和贷方。
**解决：** 从第一天起记录成本，FMS 报表、利润中心 P&L、毛利分析全部从这一张表查询。

---

## 3. 销售模块 (CRM)

### 业务流转

```
Quotation ──DERIVED_FROM──→ SalesOrder ──TRIGGERS──→ ShippingRequest ──RECONCILES──→ Reconciliation
     ↗ SalesReturn（从 ShippingRequest 逆向触发）
```

### 3.1 Quotation 报价单

```
entity Quotation {
  id: UUID PK
  doc_number: String            // QUO-2026-05-xxxxx
  customer_id: UUID FK→customers
  contact_id: UUID FK→contacts
  quotation_date: Date
  valid_until: Date
  status: QuotationStatus
  total_amount: Decimal(10,6)
  total_cost: Decimal(10,6)
  estimated_margin: Decimal(5,2)  // 预估毛利率%
  payment_terms: String
  delivery_terms: String
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity QuotationItem {
  id: UUID PK
  quotation_id: UUID FK→quotations
  line_no: i32
  product_id: UUID FK→products
  description: String
  quantity: Decimal(10,6)
  unit: String
  unit_price: Decimal(10,6)
  unit_cost: Decimal(10,6)       // BOM展开成本
  discount_rate: Decimal(5,2)
  amount: Decimal(10,6)
  delivery_date: Date?
}

enum QuotationStatus { DRAFT, SENT, ACCEPTED, REJECTED, EXPIRED }
```

**共享层：** DocumentSequence 编号。报价阶段不产生实际成本。

### 3.2 SalesOrder 销售订单

```
entity SalesOrder {
  id: UUID PK
  doc_number: String            // SO-2026-05-xxxxx
  customer_id: UUID FK→customers
  contact_id: UUID FK→contacts
  order_date: Date
  status: SalesOrderStatus
  total_amount: Decimal(10,6)
  payment_terms: String
  delivery_address: String
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity SalesOrderItem {
  id: UUID PK
  order_id: UUID FK→sales_orders
  line_no: i32
  product_id: UUID FK→products
  description: String
  quantity: Decimal(10,6)
  unit_price: Decimal(10,6)
  amount: Decimal(10,6)
  delivered_qty: Decimal(10,6)
  returned_qty: Decimal(10,6)
  delivery_date: Date?
}

enum SalesOrderStatus { DRAFT, CONFIRMED, IN_PRODUCTION, PARTIALLY_SHIPPED, SHIPPED, COMPLETED, CANCELLED }
```

**共享层：** DocumentSequence 编号 | InventoryReservation SOFT 预留 | DocumentLink 关联报价（DERIVED_FROM）

### 3.3 ShippingRequest 发货申请

```
entity ShippingRequest {
  id: UUID PK
  doc_number: String            // SR-2026-05-xxxxx
  order_id: UUID FK→sales_orders
  customer_id: UUID FK→customers
  request_date: Date
  expected_ship_date: Date
  status: ShippingStatus
  shipping_address: String
  carrier: String?
  tracking_number: String?
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity ShippingRequestItem {
  id: UUID PK
  request_id: UUID FK→shipping_requests
  order_item_id: UUID FK→sales_order_items
  product_id: UUID FK→products
  requested_qty: Decimal(10,6)
  shipped_qty: Decimal(10,6)
  warehouse_id: UUID FK→warehouses
}

enum ShippingStatus { DRAFT, CONFIRMED, PICKING, SHIPPED, CANCELLED }
```

**共享层：** InventoryReservation 释放预留 | DocumentLink 关联订单（TRIGGERS） | CostEntry 记销售成本

### 3.4 SalesReturn 销售退货

```
entity SalesReturn {
  id: UUID PK
  doc_number: String            // SRT-2026-05-xxxxx
  order_id: UUID FK→sales_orders
  shipping_request_id: UUID FK→shipping_requests
  customer_id: UUID FK→customers
  return_date: Date
  status: ReturnStatus
  return_reason: String
  total_amount: Decimal(10,6)
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity SalesReturnItem {
  id: UUID PK
  return_id: UUID FK→sales_returns
  order_item_id: UUID FK→sales_order_items
  product_id: UUID FK→products
  returned_qty: Decimal(10,6)
  unit_price: Decimal(10,6)
  amount: Decimal(10,6)
  disposition: ReturnDisposition
}

enum ReturnStatus { DRAFT, CONFIRMED, RECEIVED, INSPECTING, COMPLETED, CANCELLED }
enum ReturnDisposition { RESTOCK, SCRAP, REWORK }
```

**共享层：** DocumentLink 关联原发货（REFERENCES） | CostEntry 冲减收入

### 3.5 Reconciliation 月对账单

```
entity Reconciliation {
  id: UUID PK
  doc_number: String            // REC-2026-05-xxxxx
  customer_id: UUID FK→customers
  period: String                // "2026-05"
  status: ReconciliationStatus
  total_amount: Decimal(10,6)
  confirmed_amount: Decimal(10,6)
  difference: Decimal(10,6)     // 差异自动标记
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity ReconciliationItem {
  id: UUID PK
  reconciliation_id: UUID FK→reconciliations
  shipping_request_id: UUID FK→shipping_requests
  product_id: UUID FK→products
  quantity: Decimal(10,6)
  unit_price: Decimal(10,6)
  amount: Decimal(10,6)
  confirmed: bool
  remark: String?
}

enum ReconciliationStatus { DRAFT, SENT, CONFIRMED, DISPUTED, SETTLED }
```

**共享层：** DocumentSequence 编号 | DocumentLink 关联发货单（RECONCILES） | CostEntry 生成应收账款凭证

---

## 4. 采购模块 (SRM)

### 业务流转

```
Supplier → PurchaseQuotation(比价) → PurchaseOrder → PurchaseReconciliation → PaymentRequest
  ↗ PurchaseReturn（逆向）
  ↗ MiscellaneousRequest（零星请购）
```

### 4.1 Supplier 供应商

```
entity Supplier {
  id: UUID PK
  code: String
  name: String
  short_name: String?
  category: SupplierCategory
  status: SupplierStatus
  tax_number: String?
  legal_person: String?
  registered_capital: Decimal?
  business_scope: String?
  qualification_expiry: Date?
  lead_time_days: i32
  payment_terms: String?
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity SupplierContact {
  id: UUID PK
  supplier_id: UUID FK→suppliers
  name: String
  position: String?
  phone: String?
  email: String?
  is_primary: bool
}

entity SupplierBankAccount {
  id: UUID PK
  supplier_id: UUID FK→suppliers
  bank_name: String
  account_name: String
  account_number: String
  is_default: bool
}

enum SupplierCategory { RAW_MATERIAL, PACKAGING, OUTSOURCING, CONSUMABLE, SERVICE }
enum SupplierStatus { PROSPECTIVE, QUALIFIED, PROBATION, DISQUALIFIED, BLACKLISTED }
```

### 4.2 PurchaseQuotation 采购报价

```
entity PurchaseQuotation {
  id: UUID PK
  doc_number: String            // PQ-2026-05-xxxxx
  supplier_id: UUID FK→suppliers
  quotation_date: Date
  valid_from: Date
  valid_until: Date
  status: PurchaseQuotationStatus
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity PurchaseQuotationItem {
  id: UUID PK
  quotation_id: UUID FK→purchase_quotations
  product_id: UUID FK→products
  line_no: i32
  unit_price: Decimal(10,6)
  min_order_qty: Decimal(10,6)?
  lead_time_days: i32?
  currency: String
  is_preferred: bool
}

enum PurchaseQuotationStatus { DRAFT, ACTIVE, EXPIRED, CANCELLED }
```

### 4.3 PurchaseOrder 采购订单

```
entity PurchaseOrder {
  id: UUID PK
  doc_number: String            // PO-2026-05-xxxxx
  supplier_id: UUID FK→suppliers
  order_date: Date
  expected_delivery_date: Date?
  status: PurchaseOrderStatus
  total_amount: Decimal(10,6)
  payment_terms: String?
  delivery_address: String?
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity PurchaseOrderItem {
  id: UUID PK
  order_id: UUID FK→purchase_orders
  line_no: i32
  product_id: UUID FK→products
  description: String
  quantity: Decimal(10,6)
  unit_price: Decimal(10,6)
  amount: Decimal(10,6)
  received_qty: Decimal(10,6)
  inspected_qty: Decimal(10,6)
  returned_qty: Decimal(10,6)
  quotation_item_id: UUID? FK→purchase_quotation_items
  expected_delivery_date: Date?
}

enum PurchaseOrderStatus { DRAFT, CONFIRMED, PARTIALLY_RECEIVED, RECEIVED, CLOSED, CANCELLED }
```

**共享层：** DocumentSequence | InventoryReservation 到货后释放安全库存预留 | DocumentLink 关联报价 | CostEntry 记材料成本

### 4.4 PurchaseReturn 采购退货

```
entity PurchaseReturn {
  id: UUID PK
  doc_number: String            // PRT-2026-05-xxxxx
  order_id: UUID FK→purchase_orders
  supplier_id: UUID FK→suppliers
  return_date: Date
  status: PurchaseReturnStatus
  return_reason: String
  total_amount: Decimal(10,6)
  remark: String
  operator_id: UUID FK→users
  created_at, updated_at, deleted_at: Timestamp?
}

entity PurchaseReturnItem {
  id: UUID PK
  return_id: UUID FK→purchase_returns
  order_item_id: UUID FK→purchase_order_items
  product_id: UUID FK→products
  returned_qty: Decimal(10,6)
  unit_price: Decimal(10,6)
  amount: Decimal(10,6)
}

enum PurchaseReturnStatus { DRAFT, CONFIRMED, SHIPPED, REFUNDED, CANCELLED }
```

### 4.5 PurchaseReconciliation 对账单

```
entity PurchaseReconciliation {
  id: UUID PK
  doc_number: String
  supplier_id: UUID FK→suppliers
  period: String                // "2026-05"
  status: ReconciliationStatus
  total_amount: Decimal(10,6)
  confirmed_amount: Decimal(10,6)
  difference: Decimal(10,6)
  remark: String
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

entity PurchaseReconItem {
  id: UUID PK
  reconciliation_id: UUID
  order_id: UUID
  order_item_id: UUID
  received_qty: Decimal(10,6)
  unit_price: Decimal(10,6)
  amount: Decimal(10,6)
  confirmed: bool
}
```

### 4.6 PaymentRequest 付款申请

```
entity PaymentRequest {
  id: UUID PK
  doc_number: String
  supplier_id: UUID FK→suppliers
  reconciliation_id: UUID? FK→purchase_reconciliations
  payment_date: Date
  amount: Decimal(10,6)
  status: PaymentStatus
  payment_method: PaymentMethod
  bank_account_id: UUID? FK→supplier_bank_accounts
  invoice_number: String?
  invoice_amount: Decimal(10,6)?
  remark: String
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

enum PaymentStatus { DRAFT, APPROVED, PAID, CANCELLED }
enum PaymentMethod { BANK_TRANSFER, CASH, NOTE }
```

### 4.7 MiscellaneousRequest 零星请购

```
entity MiscellaneousRequest {
  id: UUID PK
  doc_number: String
  department_id: UUID FK→departments
  request_date: Date
  status: MiscRequestStatus
  total_amount: Decimal(10,6)
  purpose: String
  remark: String
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

entity MiscRequestItem {
  id: UUID PK
  request_id: UUID
  line_no: i32
  item_name: String
  specification: String?
  quantity: Decimal(10,6)
  unit: String
  estimated_price: Decimal(10,6)?
  remark: String?
}

enum MiscRequestStatus { DRAFT, APPROVED, PURCHASING, RECEIVED, CLOSED, CANCELLED }
```

---

## 5. 仓储模块 (WMS)

### 三级库位模型

```
Warehouse 1:N→ Zone 1:N→ Bin
库存精确到 物料 × 批次 × 储位 粒度
```

### 5.1 Warehouse 仓库

```
entity Warehouse {
  id: UUID PK
  code: String
  name: String
  warehouse_type: WarehouseType
  status: WarehouseStatus
  address: String?
  manager_id: UUID? FK→users
  is_virtual: bool              // 虚拟仓(委外)
  remark: String
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

enum WarehouseType { RAW_MATERIAL, FINISHED_GOODS, SEMI_FINISHED, CONSUMABLE, VIRTUAL_OUTSOURCE }
enum WarehouseStatus { ACTIVE, INACTIVE }
```

### 5.2 Zone 库区

```
entity Zone {
  id: UUID PK
  warehouse_id: UUID FK→warehouses
  code: String
  name: String
  zone_type: ZoneType
  sort_order: i32
  remark: String?
  created_at, updated_at, deleted_at: Timestamp?
}

enum ZoneType { RECEIVING, STORAGE, PICKING, PACKING, INSPECTION, RETURNS }
```

### 5.3 Bin 储位

```
entity Bin {
  id: UUID PK
  zone_id: UUID FK→zones
  code: String
  name: String
  row_no, column_no, layer_no: String?
  capacity_limit: Decimal?
  allowed_product_types: String[]?
  temperature_req: String?
  status: BinStatus
  created_at, updated_at, deleted_at: Timestamp?
}

enum BinStatus { EMPTY, OCCUPIED, LOCKED, DISABLED }
```

### 5.4 StockLedger 库存账

```
entity StockLedger {
  id: UUID PK
  product_id: UUID FK→products
  warehouse_id: UUID FK→warehouses
  zone_id: UUID FK→zones
  bin_id: UUID FK→bins
  batch_no: String?
  quantity: Decimal(10,6)       // 现有量
  reserved_qty: Decimal(10,6)   // 已预留量（反范式冗余，定时校准）
  available_qty: Decimal(10,6)  // = 现有量 - 已预留量
  unit_cost: Decimal(10,6)?     // 移动加权平均成本
  received_date: Date?
  expiry_date: Date?
  updated_at: Timestamp
}
```

### 5.5 策略引擎

```
entity PutawayStrategy {
  id: UUID PK
  name: String
  strategy_type: PutawayType
  warehouse_id: UUID?           // null=全局
  product_category_id: UUID?
  priority: i32
  is_active: bool
}

enum PutawayType { SAME_MERGE, NEAREST, FIXED_BIN, EMPTY_FIRST }

entity PickStrategy {
  id: UUID PK
  name: String
  strategy_type: PickType
  warehouse_id: UUID?
  priority: i32
  is_active: bool
}

enum PickType { FIFO, FEFO, SHORTEST_PATH, FULL_PALLET }
```

### 5.6 ArrivalNotice 来料通知

```
entity ArrivalNotice {
  id: UUID PK
  doc_number: String            // AN-2026-05-xxxxx
  purchase_order_id: UUID? FK→purchase_orders
  supplier_id: UUID FK→suppliers
  arrival_date: Date
  status: ArrivalStatus
  warehouse_id: UUID FK→warehouses
  zone_id: UUID? FK→zones       // 待检区
  delivery_note: String?
  remark: String
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

entity ArrivalNoticeItem {
  id: UUID PK
  notice_id: UUID
  order_item_id: UUID?
  product_id: UUID
  declared_qty: Decimal(10,6)   // 供应商申报量
  received_qty: Decimal(10,6)   // 实收量
  accepted_qty: Decimal(10,6)   // 检验合格量
  batch_no: String?
}

enum ArrivalStatus { DRAFT, RECEIVED, INSPECTING, ACCEPTED, PARTIALLY_ACCEPTED, REJECTED }
```

### 5.7 InventoryTransaction 库存事务

```
entity InventoryTransaction {
  id: UUID PK
  doc_number: String?
  transaction_type: TransactionType
  product_id: UUID
  warehouse_id: UUID
  zone_id, bin_id: UUID?
  batch_no: String?
  quantity: Decimal(10,6)       // +入库/-出库
  unit_cost: Decimal(10,6)?
  source_type: DocumentType
  source_id: UUID
  remark: String?
  operator_id: UUID
  created_at: Timestamp
}

enum TransactionType {
  PURCHASE_RECEIPT, PRODUCTION_RECEIPT,
  SALES_SHIPMENT, MATERIAL_ISSUE, MATERIAL_RETURN,
  BACKFLUSH, TRANSFER, FORM_CONVERSION,
  ADJUSTMENT, LOCK, UNLOCK, SCRAP
}
```

### 5.8 MaterialRequisition 领料单

```
entity MaterialRequisition {
  id: UUID PK
  doc_number: String            // MR-2026-05-xxxxx
  work_order_id: UUID FK→work_orders
  requisition_date: Date
  status: RequisitionStatus
  warehouse_id: UUID
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

entity MaterialReqItem {
  id: UUID PK
  requisition_id: UUID
  product_id: UUID
  requested_qty: Decimal        // BOM定额
  issued_qty: Decimal           // 实领量
  variance_qty: Decimal         // 差异量
  bin_id: UUID?
}

enum RequisitionStatus { DRAFT, CONFIRMED, ISSUED, CANCELLED }
```

### 5.9 BackflushRecord 倒冲记录

```
entity BackflushRecord {
  id: UUID PK
  doc_number: String
  work_order_id: UUID FK→work_orders
  product_id: UUID FK→products  // 完工品
  completed_qty: Decimal
  backflush_date: Date
  status: BackflushStatus
  variance_threshold: Decimal   // 差异阈值%
  operator_id: UUID
  created_at, updated_at: Timestamp
}

entity BackflushItem {
  id: UUID PK
  record_id: UUID
  component_id: UUID            // BOM子件
  theoretical_qty: Decimal      // BOM理论用量
  actual_qty: Decimal           // 实际倒冲量
  variance_qty: Decimal
  variance_rate: Decimal        // 差异率%
  is_over_threshold: bool
}

enum BackflushStatus { DRAFT, EXECUTED, ADJUSTED }
```

### 5.10 CycleCount 循环盘点

```
entity CycleCount {
  id: UUID PK
  doc_number: String
  warehouse_id: UUID
  zone_id: UUID?
  count_date: Date
  status: CycleCountStatus
  is_blind: bool                // 盲盘
  remark: String?
  operator_id: UUID
  created_at, updated_at: Timestamp
}

entity CycleCountItem {
  id: UUID PK
  count_id: UUID
  bin_id: UUID
  product_id: UUID
  batch_no: String?
  system_qty: Decimal           // 盲盘时隐藏
  counted_qty: Decimal
  variance_qty: Decimal
  variance_reason: String?
  is_adjusted: bool
}

enum CycleCountStatus { DRAFT, COUNTING, COMPLETED, ADJUSTED, CANCELLED }
```

### 5.11 InventoryTransfer 库存调拨

```
entity InventoryTransfer {
  id: UUID PK
  doc_number: String
  from_warehouse_id, from_zone_id, from_bin_id: UUID?
  to_warehouse_id, to_zone_id, to_bin_id: UUID?
  transfer_date: Date
  status: TransferStatus
  operator_id: UUID
  created_at: Timestamp
}

entity TransferItem {
  id: UUID PK
  transfer_id: UUID
  product_id: UUID
  quantity: Decimal
  batch_no: String?
}

enum TransferStatus { DRAFT, IN_TRANSIT, COMPLETED, CANCELLED }
```

### 5.12 FormConversion 形态转换

```
entity FormConversion {
  id: UUID PK
  doc_number: String
  warehouse_id: UUID
  conversion_date: Date
  status: ConversionStatus
  remark: String
  operator_id: UUID
  created_at: Timestamp
}

entity ConversionItem {
  id: UUID PK
  conversion_id: UUID
  direction: ConversionDir
  product_id: UUID
  quantity: Decimal
  unit_cost: Decimal
  batch_no: String?
}

enum ConversionDir { CONSUME, PRODUCE }
enum ConversionStatus { DRAFT, COMPLETED, CANCELLED }
```

### 5.13 InventoryLock 库存锁定

```
entity InventoryLock {
  id: UUID PK
  doc_number: String
  product_id: UUID
  warehouse_id: UUID
  locked_qty: Decimal
  lock_reason: String
  customer_id: UUID? FK→customers
  status: LockStatus
  operator_id: UUID
  created_at, updated_at: Timestamp
}

enum LockStatus { ACTIVE, RELEASED, CANCELLED }
```

---

## 6. 生产模块 (MES)

### 业务流转

```
ProductionPlan → WorkOrder → WorkOrderRouting(工序) → WorkReport(报工) → ProductionReceipt(完工入库)
  ↗ OutsourcingOrder（委外=虚拟库位）
  ↗ ProductionInspection（生产报检）
```

### 6.1 ProductionPlan 生产计划

```
entity ProductionPlan {
  id: UUID PK
  doc_number: String            // PP-2026-05-xxxxx
  plan_date: Date
  plan_type: PlanType
  status: PlanStatus
  remark: String
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

entity ProductionPlanItem {
  id: UUID PK
  plan_id: UUID FK→production_plans
  product_id: UUID FK→products
  planned_qty: Decimal(10,6)
  scheduled_start: Date
  scheduled_end: Date
  sales_order_id: UUID? FK→sales_orders          // MTO
  sales_order_item_id: UUID?
  bom_snapshot_id: UUID?                          // BOM版本快照
  routing_id: UUID? FK→labor_processes
  work_center_id: UUID?
  priority: i32
  status: PlanItemStatus
}

enum PlanType { MTO, MTS }
enum PlanStatus { DRAFT, CONFIRMED, IN_PROGRESS, COMPLETED, CANCELLED }
enum PlanItemStatus { PLANNED, RELEASED, IN_PRODUCTION, COMPLETED, CANCELLED }
```

### 6.2 WorkOrder 工单

```
entity WorkOrder {
  id: UUID PK
  doc_number: String            // WO-2026-05-xxxxx
  plan_item_id: UUID? FK→production_plan_items
  product_id: UUID FK→products
  bom_snapshot_id: UUID?                          // 冻结的BOM快照
  routing_id: UUID? FK→labor_processes
  planned_qty: Decimal(10,6)
  completed_qty: Decimal(10,6)
  scrap_qty: Decimal(10,6)
  scheduled_start: Date
  scheduled_end: Date
  actual_start: Timestamp?
  actual_end: Timestamp?
  status: WorkOrderStatus
  work_center_id: UUID?
  team_id: UUID?
  sales_order_id: UUID?                           // MTO关联
  remark: String
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

enum WorkOrderStatus { DRAFT, PLANNED, RELEASED, IN_PRODUCTION, COMPLETED, CLOSED, CANCELLED }
```

### 6.3 WorkOrderRouting 工单工序

```
entity WorkOrderRouting {
  id: UUID PK
  work_order_id: UUID FK→work_orders
  step_no: i32
  process_name: String
  work_center_id: UUID?
  standard_time: Decimal?       // 标准工时(小时)
  standard_cost: Decimal?       // 标准人工成本
  planned_qty: Decimal(10,6)
  completed_qty: Decimal(10,6)
  defect_qty: Decimal(10,6)
  status: RoutingStatus
  is_outsourced: bool
  is_inspection_point: bool
}

enum RoutingStatus { PENDING, IN_PROGRESS, COMPLETED, SKIPPED }
```

### 6.4 WorkReport 报工记录

```
entity WorkReport {
  id: UUID PK
  doc_number: String            // WR-2026-05-xxxxx
  work_order_id: UUID FK→work_orders
  routing_id: UUID FK→work_order_routings
  report_date: Date
  shift: ShiftType
  worker_id: UUID FK→users
  completed_qty: Decimal(10,6)
  defect_qty: Decimal(10,6)
  defect_reason: String?
  work_hours: Decimal
  remark: String
  operator_id: UUID
  created_at, updated_at: Timestamp
}

enum ShiftType { DAY, NIGHT }
```

### 6.5 OutsourcingOrder 委外单

```
entity OutsourcingOrder {
  id: UUID PK
  doc_number: String            // OO-2026-05-xxxxx
  work_order_id: UUID? FK→work_orders
  routing_id: UUID? FK→work_order_routings
  supplier_id: UUID FK→suppliers
  product_id: UUID
  outsourcing_type: OutsourcingType
  planned_qty: Decimal(10,6)
  completed_qty: Decimal(10,6)
  unit_price: Decimal(10,6)
  scheduled_date: Date?
  status: OutsourcingStatus
  virtual_warehouse_id: UUID    // WMS虚拟库位
  remark: String
  operator_id: UUID
  created_at, updated_at, deleted_at: Timestamp?
}

enum OutsourcingType { FULL, PROCESS, MATERIAL, REWORK }
enum OutsourcingStatus { DRAFT, SENT, IN_PRODUCTION, DELIVERED, RECEIVED, CLOSED, CONVERTED_TO_INTERNAL, CANCELLED }
```

**委外 = 虚拟库位模型：** 发外协 = 调拨到虚拟仓 | 外协收货 = 从虚拟仓调回 | 转自制 = 工单移回内部

### 6.6 ProductionInspection 生产报检

```
entity ProductionInspection {
  id: UUID PK
  doc_number: String            // PI-2026-05-xxxxx
  work_order_id: UUID
  routing_id: UUID?
  product_id: UUID
  inspection_type: InspectionType
  sample_qty: Decimal(10,6)
  qualified_qty: Decimal(10,6)
  unqualified_qty: Decimal(10,6)
  result: InspectionResult
  inspector_id: UUID
  inspection_date: Date
  disposition: String?
  remark: String
  operator_id: UUID
  created_at, updated_at: Timestamp
}

enum InspectionType { FIRST_ARTICLE, IN_PROCESS, FINAL }
enum InspectionResult { PASS, FAIL, CONDITIONAL }
```

### 6.7 ProductionReceipt 完工入库

```
entity ProductionReceipt {
  id: UUID PK
  doc_number: String            // PR-2026-05-xxxxx
  work_order_id: UUID FK→work_orders
  product_id: UUID
  received_qty: Decimal(10,6)
  warehouse_id: UUID
  zone_id, bin_id: UUID?
  receipt_date: Date
  status: ReceiptStatus
  backflush_triggered: bool
  remark: String
  operator_id: UUID
  created_at, updated_at: Timestamp
}

enum ReceiptStatus { DRAFT, CONFIRMED, CANCELLED }
```

**共享层：** DocumentSequence 编号 | InventoryReservation 工单下达 HARD 预留 → 领料消耗 → 完工释放 | CostEntry 报工记人工/制费、完工记成品入库、倒冲差异记损耗

---

## 7. 跨模块交互矩阵

| 事件 | DocumentSequence | DocumentLink | InventoryReservation | CostEntry |
|------|-----------------|--------------|---------------------|-----------|
| 销售订单确认 | SO 编号 | 关联报价 | SOFT 预留 | — |
| 发货申请确认 | SR 编号 | 关联订单 | 释放 SOFT | 销售成本 |
| 采购订单确认 | PO 编号 | 关联报价 | — | — |
| 来料检验合格 | AN 编号 | 关联采购单 | — | 材料成本 |
| 工单下达 | WO 编号 | 关联计划 | HARD 预留 | — |
| 领料出库 | MR 编号 | 关联工单 | 消耗 HARD | — |
| 报工 | WR 编号 | 关联工单 | — | 人工/制费 |
| 完工入库 | PR 编号 | 关联工单 | 释放剩余 | 成品入库 |
| 倒冲执行 | BF 编号 | 关联工单 | — | 损耗成本 |
| 对账确认 | REC 编号 | 关联发货/入库 | — | 应收/应付 |
| 付款 | PAY 编号 | 关联对账 | — | 资金流出 |

## 8. 视觉化文档

HTML 格式的交互式类图保存在 `docs/uml-design/` 目录：
- `00-shared-infrastructure.html` — 共享层
- `01-sales.html` — 销售模块
- `02-purchase.html` — 采购模块
- `03-wms.html` — 仓储模块
- `04-mes.html` — 生产模块
