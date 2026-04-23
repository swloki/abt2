# 功能：根据工艺路线查询引用的 BOM 列表

## 需求

前端需要在工艺路线详情页查看所有引用该路线的 BOM 列表，支持分页查询。

## 接口设计

### gRPC 接口

**Rpc**: `GetBomsByRouting(GetBomsByRoutingRequest) returns (BomListByRoutingResponse)`

归属 `AbtRoutingService`。

### 消息定义

```protobuf
message GetBomsByRoutingRequest {
  int64 routing_id = 1;
  optional uint32 page = 2;
  optional uint32 page_size = 3;
}

message BomBriefProto {
  int64 bom_id = 1;
  string bom_name = 2;
  string created_at = 3;
}

message BomListByRoutingResponse {
  repeated BomBriefProto items = 1;
  uint64 total = 2;
}
```

### 数据流

1. Handler 接收 `routing_id` + 分页参数
2. Service 调用 Repo 查询
3. Repo 执行 SQL：
   ```sql
   -- 通过 bom_routing 表找到绑定了该 routing_id 的 product_code
   -- 再通过 bom 表的 bom_detail->nodes 找到根节点匹配该 product_code 的 BOM
   SELECT b.bom_id, b.bom_name, b.create_at
   FROM bom_routing br
   JOIN bom b ON EXISTS (
     SELECT 1 FROM jsonb_array_elements(b.bom_detail->'nodes') AS node
     WHERE (node->>'product_id')::bigint IN (
       SELECT p.product_id FROM products p
       WHERE p.meta->>'product_code' = br.product_code
     )
     AND (node->>'parent_id')::bigint = 0
   )
   WHERE br.routing_id = $1
   ORDER BY b.bom_id DESC
   LIMIT $2 OFFSET $3
   ```
4. 单独查 COUNT 做分页

## 分层改动

1. **Proto** (`proto/abt/v1/routing.proto`) — 新增消息和 rpc
2. **Repo** (`abt/src/repositories/routing_repo.rs`) — 新增 `find_boms_by_routing_id`
3. **Service trait** (`abt/src/service/routing_service.rs`) — 新增 `list_boms_by_routing`
4. **Service impl** (`abt/src/implt/routing_service_impl.rs`) — 实现
5. **Handler** (`abt-grpc/src/handlers/routing.rs`) — 新增 `get_boms_by_routing`

## 权限

复用 `Resource::Routing, Action::Read`，与现有路由查看权限一致。
