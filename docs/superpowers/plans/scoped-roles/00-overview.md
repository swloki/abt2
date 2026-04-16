# Scoped Roles Implementation Plan — Overview

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce department-scoped role assignments so users can have different roles (and thus different permissions) in different departments.

**Architecture:** Replace the global `user_roles` table with `user_department_roles` (user→department→role mapping). JWT stores dept-role mappings instead of flat permissions. A runtime permission cache resolves role inheritance and provides fast lookups. The `require_permission` macro internally changes to a two-step check (department resource visibility + scoped role permissions) while its call-site signature stays the same.

**Tech Stack:** Rust, sqlx (PostgreSQL), tonic (gRPC), jsonwebtoken, serde

**Spec:** `docs/superpowers/specs/2026-04-16-scoped-roles-design.md`

## Dependency Graph

```
01-database-migration (must be first)
        |
        v
02-models-and-repos ──┐
03-permission-cache  ──┤  (parallel after 01)
        |              |
        v              v
04-auth-jwt (needs 02 + 03)
        |
        v
05-macro-update (needs 04)
        |
        v
06-proto-grpc (needs 05)
```

## Plan Files

| File | Scope | Depends on |
|---|---|---|
| `01-database-migration.md` | New table + column + seed data | Nothing |
| `02-models-and-repos.md` | DeptRole model + UserDepartmentRoleRepo | 01 |
| `03-permission-cache.md` | RolePermissionCache with inheritance + cycle detection | 01 |
| `04-auth-jwt.md` | Claims/AuthContext refactor, login/refresh/update | 02 + 03 |
| `05-macro-update.md` | require_permission macro internal changes | 04 |
| `06-proto-grpc.md` | Proto, service trait, handler changes | 05 |
