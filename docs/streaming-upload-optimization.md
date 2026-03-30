# Excel 上传流式转发优化

## 状态：已实现 ✅

前端上传文件时，不再保存到本地，直接流式转发给后端。

## 架构

```
前端 → Astro API → Rust gRPC (UploadFile) → 返回 filePath → Astro Action (ImportExcel)
```

## 实现细节

### 1. 流式上传 API (`src/pages/api/sync/upload.ts`)

使用 `@bufbuild/protobuf` 的 `create` 构造消息，通过 gRPC client streaming 发送给后端：

```typescript
import { create } from "@bufbuild/protobuf";
import { excelService } from "@/lib/grpc-client";
import { UploadFileRequestSchema } from "@buf/xweichen_abt.bufbuild_es/abt/v1/excel_pb";

async function* generateRequests() {
  // 先发送文件名
  yield create(UploadFileRequestSchema, {
    data: { case: "fileName", value: file.name },
  });
  // 再发送文件内容（分块）
  const chunkSize = 64 * 1024; // 64KB per chunk
  for (let i = 0; i < uint8Array.length; i += chunkSize) {
    const chunk = uint8Array.slice(i, i + chunkSize);
    yield create(UploadFileRequestSchema, {
      data: { case: "chunk", value: chunk },
    });
  }
}

const response = await excelService.uploadFile(generateRequests());
// response.filePath 是后端返回的文件路径
```

### 2. Proto 定义 (`abt/v1/excel.proto`)

```protobuf
message UploadFileRequest {
  oneof data {
    string file_name = 1;
    bytes chunk = 2;
  }
}

message UploadFileResponse {
  string file_path = 1;  // 上传后的文件路径
  int64 file_size = 2;
}

service AbtExcelService {
  rpc UploadFile(stream UploadFileRequest) returns (UploadFileResponse);
  rpc ImportExcel(ImportExcelRequest) returns (ImportResultResponse);
  // ...
}
```

### 3. 后端 Rust 处理 (`abt-grpc/src/handlers/excel.rs`)

```rust
async fn upload_file(
    &self,
    request: Request<StreamingRequest<UploadFileRequest>>,
) -> GrpcResult<UploadFileResponse> {
    // 流式接收文件内容，写入临时目录
    // 返回文件路径供后续 ImportExcel 使用
}
```

### 4. 导入 Action (`src/actions/sync.ts`)

```typescript
import: defineAction({
  input: z.object({
    filePath: z.string().min(1, '文件路径不能为空'),
  }),
  handler: async (input) => {
    const result = await excelService.importExcel({ filePath: input.filePath });
    return {
      successCount: result.successCount,
      failedCount: result.failedCount,
      errors: result.errors
    };
  },
}),
```

## 已删除的配置

- ~~`UPLOAD_TEMP_DIR`~~ 环境变量（不再需要本地存储）
- ~~本地文件保存逻辑~~

## 内存优化效果

- **之前**：完整文件读入内存 O(n)，大文件（50MB+）导致 Astro 内存飙升
- **之后**：边收边转发 O(1)，内存占用稳定在 64KB/块

## 待优化：同步进度

当前 `getProgress` 接口始终返回 `0/0`，因为 `ProductExcelServiceImpl` 每次调用 `get_product_excel_service` 都创建新实例，计数器被重置。

详见：[进度问题文档](./progress-bug.md)（需后端修复）
