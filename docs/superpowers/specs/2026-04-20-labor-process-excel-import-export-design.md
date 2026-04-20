# 工序 Excel 导入导出设计

日期: 2026-04-20

## 背景

人工成本系统已完成从扁平结构到三层模型的重构（迁移 021），工序管理功能（CRUD）已上线，但 Excel 批量导入导出功能尚未适配新模型。用户需要通过 Excel 快速批量创建和更新工序数据。

## 需求

1. **导入工序**：通过 Excel 批量导入工序（名称、单价、备注），已存在的工序按名称匹配并更新，不存在的新增（upsert）
2. **导出工序**：导出现有工序列表到 Excel，用户可在此基础上修改后重新导入
3. **Excel 格式统一**：导入和导出使用相同的列格式

## Excel 格式

表头（第 1 行）：

| 工序名称 | 单价 | 备注 |
|---------|------|------|
| 切割 | 15.50 | 激光切割 |
| 焊接 | 25.00 | |

- **工序名称**（必填）：用于匹配已有工序，名称相同则更新
- **单价**（必填）：Decimal 精度
- **备注**（选填）：文本

## 技术方案

扩展现有的 `LaborProcessService`，在其 trait 和 impl 中添加 Excel 导入/导出方法。不创建新的独立 Excel 服务。

选择此方案的原因：工序导入只有 3 个字段，逻辑简单，不需要像产品导入那样的进度追踪和复杂匹配。

## gRPC 接口设计

为工序导入/导出创建独立的 proto RPC 方法，而非通过字符串在共享端点中派发：

```protobuf
// 导入工序
rpc ImportLaborProcesses(ImportLaborProcessesRequest) returns (ImportLaborProcessesResponse);

// 导出工序
rpc ExportLaborProcesses(ExportLaborProcessesRequest) returns (stream DownloadFileResponse);
```

**选择独立 RPC 的理由：** 字符串派发（`import_type = "labor_processes"`）是耦合磁铁——每个新导入类型都在已有代码中添加分支。独立 RPC 使功能自包含、独立测试，且 proto 定义量很小（~20 行）。对于简单功能，最简单的架构是不与复杂功能共享机制。

## 导入流程

### 解析与规范化

```
1. 读取 Excel 文件（calamine 解析）
2. 跳过表头，逐行解析（工序名称、单价、备注）
3. 名称规范化（解析后立即执行）：
   - 去除首尾空白
   - 全角空格（U+3000）→ 半角空格
   - 全角括号（（））→ 半角括号
   - 移除零宽字符
   - 规范化后再进行 upsert 匹配
```

**名称规范化的理由：** 中文 Excel 中的全半角差异是数据脏化的高频来源（product import 历史痛点）。"组装工艺(人工)"和"组装工艺（人工）"会创建幽灵重复记录。

### 数据验证

```
4. 数据验证（收集所有错误，不在第一行错误时中断）：
   - 工序名称不能为空
   - 单价必须 >= 0 且是有效的数字
   - 精度策略：存储为 Decimal(18,6)
     * 超出 6 位小数：银行家舍入（round half even），并在结果中标记为"已舍入"
     * 负数：拒绝
     * NaN / 非数字：拒绝
   - 每个错误包含：行号 + 字段名 + 错误类型 + 实际值
```

**精度策略的理由：** 单价错误是财务级问题。BOM 系统中 labor cost 直接影响成本核算。Excel 的浮点表示（如 1.4999999999999999）与数据库 Decimal(18,6) 之间的不匹配是静默错误的来源。

### Upsert 执行

```
5. 事务内执行（单事务，原子性）：
   - 使用 INSERT ... ON CONFLICT (name) DO UPDATE SET
     unit_price = EXCLUDED.unit_price,
     remark = EXCLUDED.remark,
     updated_at = NOW()
   - 返回每行的执行结果（created / updated / unchanged）
```

**ON CONFLICT 的理由：** 项目历史经验表明"先查后写"模式有竞态条件风险（price snapshot 竞态）。ON CONFLICT 利用 name 列的 UNIQUE 约束，在数据库层面保证并发安全，不需要额外的 SELECT FOR UPDATE。

### 结果返回

```
6. 返回结果：
   - 成功数、失败数、跳过数
   - 逐行结果列表（行号、工序名称、操作类型、错误信息）
   - 受影响的 BOM 统计（单价变更时有多少 BOM 受影响）
```

### Dry-run 预览（可选）

请求中可设置 `dry_run = true`，此时：
- 执行解析、规范化、验证
- 模拟 upsert 并返回预览报告（将创建 X 行，更新 Y 行）
- **不写入数据库**
- 返回受影响 BOM 的统计信息

**Dry-run 的理由：** 单价变更会级联影响 BOM 成本快照。用户在提交前应能看到影响范围，避免意外的大规模成本重算。

## 导出流程

```
1. 查询所有未删除的工序（id, name, unit_price, remark）
2. 使用与导入相同的列定义常量生成 Excel 表头
3. 逐行写入数据
4. 返回字节流
```

**导出-导入往返保证：** 导出和导入必须共享同一个列定义（列名、列顺序），确保用户"导出 → 修改 → 重新导入"的工作流无缝衔接。具体做法：定义常量 `LABOR_PROCESS_COLUMNS = ["工序名称", "单价", "备注"]`，导出和导入共用。

## 文件变更清单

| 文件 | 变更 |
|------|------|
| `proto/abt/v1/labor_process.proto` | 添加 ImportLaborProcesses / ExportLaborProcesses RPC 和消息定义 |
| `abt/src/service/labor_process_service.rs` | 添加 import_from_excel 和 export_to_bytes 方法 |
| `abt/src/implt/labor_process_service_impl.rs` | 实现导入/导出逻辑（含规范化、验证、dry-run） |
| `abt/src/repositories/labor_process_repo.rs` | 添加基于 ON CONFLICT 的批量 upsert 查询 |
| `abt-grpc/src/handlers/labor_process.rs` | 添加导入/导出的 gRPC handler |

不需要修改：模型文件、lib.rs（无新 singleton）、数据库迁移（无新表/列）、excel.proto（使用独立 RPC 而非共享端点）。
