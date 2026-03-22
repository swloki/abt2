# RBAC 权限系统实现计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 ABT 系统实现基于角色的访问控制（RBAC）权限系统

**Architecture:** 采用标准 RBAC 模型：用户-角色-权限三层结构，支持多对多关系。超级管理员绕过权限检查，普通用户通过角色获取权限，采用"就高原则"（任一角色有权限即通过）。

**Tech Stack:** Rust, sqlx, tonic (gRPC), PostgreSQL

---

## 计划文件

按依赖顺序执行：

| # | 文件 | 内容 | 预计时间 |
|---|------|------|----------|
| 1 | [01-migration.md](./01-migration.md) | 数据库表 + 预置数据 | 30min |
| 2 | [02-models.md](./02-models.md) | 数据模型定义 | 20min |
| 3 | [03-repositories.md](./03-repositories.md) | 数据访问层 | 40min |
| 4 | [04-services.md](./04-services.md) | 业务逻辑层 | 60min |
| 5 | [05-grpc.md](./05-grpc.md) | gRPC 接口层 | 40min |

## 依赖关系

```
01-migration ──► 02-models ──► 03-repositories ──► 04-services ──► 05-grpc
```

## 快速开始

1. 确保数据库连接正常
2. 按顺序执行每个计划文件
3. 每完成一个文件，运行测试验证

## 验收标准

- [ ] 所有数据库表创建成功，包含预置数据
- [ ] Models 层编译通过，FromRow 实现正确
- [ ] Repositories 层 CRUD 操作正常
- [ ] Services 层业务逻辑完整，包含审计日志
- [ ] gRPC 接口可通过 grpcurl 测试
- [ ] 权限检查逻辑正确（超级管理员/普通用户）
