# BOM 分类功能设计

## 概述

为 BOM 增加分类属性，支持按分类划分 BOM（如电源BOM、模组BOM）。提供 BOM 分类的 CRUD 操作。

## 数据库设计

### 新建表 `bom_category`

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `bom_category_id` | `BIGSERIAL` | PRIMARY KEY | 分类 ID |
| `bom_category_name` | `VARCHAR(100)` | NOT NULL, UNIQUE | 分类名称 |
| `created_at` | `TIMESTAMPTZ` | NOT NULL, DEFAULT NOW() | 创建时间 |

### 修改表 `bom`

新增字段：

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `bom_category_id` | `BIGINT` | NULLABLE, FK -> bom_category | BOM 分类 |

## Proto 定义

### 服务定义

```proto
service AbtBomCategoryService {
  rpc ListBomCategories(ListBomCategoriesRequest) returns (BomCategoryListResponse);
  rpc GetBomCategory(GetBomCategoryRequest) returns (BomCategoryResponse);
  rpc CreateBomCategory(CreateBomCategoryRequest) returns (U64Response);
  rpc UpdateBomCategory(UpdateBomCategoryRequest) returns (BoolResponse);
  rpc DeleteBomCategory(DeleteBomCategoryRequest) returns (BoolResponse);
}
```

### 消息定义

```proto
message BomCategoryResponse {
  int64 bom_category_id = 1;
  string bom_category_name = 2;
  int64 created_at = 3;
}

message BomCategoryListResponse {
  repeated BomCategoryResponse items = 1;
  uint64 total = 2;
}

message ListBomCategoriesRequest {
  optional uint32 page = 1;
  optional uint32 page_size = 2;
  optional string keyword = 3;
}

message GetBomCategoryRequest {
  int64 bom_category_id = 1;
}

message CreateBomCategoryRequest {
  string bom_category_name = 1;
}

message UpdateBomCategoryRequest {
  int64 bom_category_id = 1;
  string bom_category_name = 2;
}

message DeleteBomCategoryRequest {
  int64 bom_category_id = 1;
}
```

### BOM 列表查询扩展

`ListBomsRequest` 新增过滤条件：

```proto
message ListBomsRequest {
  // ... 现有字段 ...
  optional int64 bom_category_id = 7;  // 按分类过滤
}
```

## 层级结构

```
abt/src/
├── models/
│   └── bom_category.rs              # BOM 分类模型
├── repositories/
│   └── bom_category_repo.rs         # BOM 分类数据访问
├── service/
│   └── bom_category_service.rs       # BOM 分类业务接口
└── implt/
    └── bom_category_impl.rs          # BOM 分类服务实现

abt-grpc/src/
├── handlers/
│   └── bom_category_handler.rs       # gRPC 处理器
└── generated/                        # 自动生成

proto/abt/v1/
└── bom_category.proto               # Proto 定义
```

## CRUD 操作说明

| 操作 | 说明 |
|------|------|
| **Create** | 创建新分类，名称唯一校验 |
| **List** | 分页列表查询，支持名称关键字过滤 |
| **Get** | 根据 ID 获取单个分类 |
| **Update** | 更新分类名称（需唯一校验） |
| **Delete** | 删除分类，如有 BOM 关联则阻止删除 |

## 实现步骤

1. 新增 proto 文件 `bom_category.proto`
2. 新增数据库迁移创建 `bom_category` 表
3. 新增数据库迁移为 `bom` 表添加 `bom_category_id` 列
4. 新增模型 `bom_category.rs`
5. 新增 Repository `bom_category_repo.rs`
6. 新增 Service Trait `bom_category_service.rs`
7. 新增 Service Impl `bom_category_impl.rs`
8. 新增 Handler `bom_category_handler.rs`
9. 在 `server.rs` 中注册 Handler
10. 在 `ListBomsRequest` 中添加 `bom_category_id` 过滤条件
