---
name: Labor Process Routing Design
date: 2026-04-22
status: approved
---

# 工艺路线管理设计

## 问题背景

当前 `bom_labor_process` 表是一个扁平模型，每个产品独立维护自己的工序列表（名称、单价、数量）。存在以下问题：

- **Excel 导入时工序遗漏**：上传的 Excel 文件可能缺少某些工序行，导致产品工序不完整
- **手动录入时工序遗漏**：用户在系统中手动添加工序时可能忘记添加必要的工序
- **缺乏校验基准**：无法判断一个产品的工序是否完整

## 设计目标

引入工序字典和工艺路线机制，为每个产品提供一个"工序基准"，在导入和录入时校验工序完整性，防止遗漏。

## 数据表设计

### 1. `labor_process_dict` — 工序字典表

全局工序主数据，维护所有工序的标准定义。

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| id | BIGSERIAL | PRIMARY KEY | 主键 |
| code | VARCHAR(50) | UNIQUE NOT NULL | 工序编码 |
| name | VARCHAR(255) | UNIQUE NOT NULL | 工序名称 |
| description | TEXT | | 说明 |
| sort_order | INT | NOT NULL DEFAULT 0 | 排序 |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | 创建时间 |
| updated_at | TIMESTAMPTZ | | 更新时间 |

### 2. `routing` — 工艺路线

可复用的工序组合模板。多个产品可共用同一条工艺路线。

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| id | BIGSERIAL | PRIMARY KEY | 主键 |
| name | VARCHAR(255) | NOT NULL | 路线名称 |
| description | TEXT | | 说明 |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | 创建时间 |
| updated_at | TIMESTAMPTZ | | 更新时间 |

### 3. `routing_step` — 路线工序明细

工艺路线中包含的具体工序，定义顺序和是否必须。

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| id | BIGSERIAL | PRIMARY KEY | 主键 |
| routing_id | BIGINT | NOT NULL | 关联路线 |
| process_code | VARCHAR(50) | NOT NULL | 关联工序编码 |
| step_order | INT | NOT NULL DEFAULT 0 | 工序顺序 |
| is_required | BOOLEAN | NOT NULL DEFAULT true | 是否必须 |
| remark | TEXT | | 备注 |

UNIQUE 约束：`(routing_id, process_code)` — 同一路线中每个工序只能出现一次。

### 4. `bom_routing` — BOM 路线映射

产品与工艺路线的绑定关系。每个产品绑定一条工艺路线。

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| id | BIGSERIAL | PRIMARY KEY | 主键 |
| product_code | VARCHAR(100) | UNIQUE NOT NULL | 产品编码 |
| routing_id | BIGINT | NOT NULL | 关联路线 |
| created_at | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | 创建时间 |
| updated_at | TIMESTAMPTZ | | 更新时间 |

### 5. `bom_labor_process` — 现有表变更

在现有表上新增 `process_code` 列。

新增列：`process_code VARCHAR(50)` — 关联到 `labor_process_dict.code`（应用层关联，不加数据库外键）。

### 关联关系图

```
labor_process_dict (全局工序字典)
       ↓ process_code 引用
routing_step (路线工序明细：工序 + 顺序 + 是否必须)
       ↓ routing_id 属于
routing (工艺路线模板，可复用)
       ↓ routing_id 绑定
bom_routing (product_code → routing_id 映射)
       ↓ 校验基准
bom_labor_process (产品实际工序 + 单价 + 数量，增加 process_code)
```

所有关联均为应用层逻辑，不使用数据库外键约束。

## 接口设计

### 工序字典 CRUD（AbtLaborProcessDictService）

- `ListLaborProcessDicts` — 分页查询，支持关键字搜索
- `CreateLaborProcessDict` — 创建工序（code + name）
- `UpdateLaborProcessDict` — 更新工序信息
- `DeleteLaborProcessDict` — 删除工序（需检查是否被路线引用）

### 工艺路线 CRUD（AbtRoutingService）

- `ListRoutings` — 分页查询路线列表
- `CreateRouting` — 创建路线（含工序明细列表）
- `UpdateRouting` — 更新路线信息及工序明细
- `DeleteRouting` — 删除路线（需检查是否被产品绑定）
- `GetRoutingDetail` — 获取路线详情（含所有工序明细）

### BOM 路线绑定

集成到现有服务中：

- `SetBomRouting` — 设置产品的工艺路线（创建或更新 `bom_routing` 记录）
- `GetBomRouting` — 查询产品的工艺路线

### 现有接口改造

`bom_labor_process` 的导入逻辑改造：

- Excel 模板增加"工序编码"列
- 导入时按路线校验完整性（见下方校验逻辑）
- 单条创建/更新工序时也支持校验

## 导入校验逻辑

### Excel 导入流程

1. 根据 `product_code` 查询 `bom_routing` 得到 `routing_id`
2. 若产品未绑定路线 → 跳过校验，正常导入（向后兼容）
3. 查询 `routing_step WHERE routing_id = ? AND is_required = true` 得到所有必须工序编码集合
4. 解析 Excel 中每行的工序编码，与必须工序集合做匹配
5. 校验结果：
   - **必须工序缺失**：报告中列出所有缺失的工序编码和名称，返回错误
   - **多余工序**（Excel 有但路线里没有）：警告提示，但仍允许导入
   - **全部必须工序匹配**：正常导入，写入 `bom_labor_process`

### 手动录入校验

添加工序时，如果产品绑定了路线，返回提示信息："该产品还需要以下工序：[列出未添加的必须工序]"。

## 向后兼容

- 产品未绑定路线时，不进行任何校验，保持现有行为
- `bom_labor_process.process_code` 列允许为空，现有数据不受影响
- 现有接口保持不变，只增加新字段

## 迁移策略

1. 新建 `labor_process_dict`、`routing`、`routing_step`、`bom_routing` 四张表
2. `bom_labor_process` 表增加 `process_code` 列（VARCHAR(50)，允许 NULL）
3. 不修改或删除现有数据
