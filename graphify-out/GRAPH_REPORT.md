# Graph Report - abt-web/src  (2026-05-26)

## Corpus Check
- 242 files · ~123,565 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 918 nodes · 1987 edges · 65 communities (59 shown, 6 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_BOM Actions & Cost|BOM Actions & Cost]]
- [[_COMMUNITY_SSR Data Loading|SSR Data Loading]]
- [[_COMMUNITY_Warehouse & Location UI|Warehouse & Location UI]]
- [[_COMMUNITY_Audit Log UI|Audit Log UI]]
- [[_COMMUNITY_Workflow Graph Engine|Workflow Graph Engine]]
- [[_COMMUNITY_Admin List Components|Admin List Components]]
- [[_COMMUNITY_Navigation & Permissions|Navigation & Permissions]]
- [[_COMMUNITY_Workflow Pages|Workflow Pages]]
- [[_COMMUNITY_BOM Cost & Issue UI|BOM Cost & Issue UI]]
- [[_COMMUNITY_Permission Configuration|Permission Configuration]]
- [[_COMMUNITY_UI Component Library|UI Component Library]]
- [[_COMMUNITY_Workflow Actions|Workflow Actions]]
- [[_COMMUNITY_Core Actions Registry|Core Actions Registry]]
- [[_COMMUNITY_Price History UI|Price History UI]]
- [[_COMMUNITY_Inventory Management UI|Inventory Management UI]]
- [[_COMMUNITY_BOM Drag & Drop|BOM Drag & Drop]]
- [[_COMMUNITY_Product Form UI|Product Form UI]]
- [[_COMMUNITY_BOM Type Definitions|BOM Type Definitions]]
- [[_COMMUNITY_Product Pages|Product Pages]]
- [[_COMMUNITY_User Management UI|User Management UI]]
- [[_COMMUNITY_Workflow Visual Editor|Workflow Visual Editor]]
- [[_COMMUNITY_Layout & Navigation|Layout & Navigation]]
- [[_COMMUNITY_Authentication|Authentication]]
- [[_COMMUNITY_Inventory Type Definitions|Inventory Type Definitions]]
- [[_COMMUNITY_Dashboard & Sidebar|Dashboard & Sidebar]]
- [[_COMMUNITY_Inventory & Location Actions|Inventory & Location Actions]]
- [[_COMMUNITY_Product & Role Actions|Product & Role Actions]]
- [[_COMMUNITY_BOM View & Cascade Inventory|BOM View & Cascade Inventory]]
- [[_COMMUNITY_User Edit Page|User Edit Page]]
- [[_COMMUNITY_Core Type Definitions|Core Type Definitions]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 39|Community 39]]
- [[_COMMUNITY_Community 40|Community 40]]
- [[_COMMUNITY_Community 41|Community 41]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 43|Community 43]]
- [[_COMMUNITY_Community 44|Community 44]]
- [[_COMMUNITY_Community 45|Community 45]]
- [[_COMMUNITY_Community 46|Community 46]]
- [[_COMMUNITY_Community 47|Community 47]]
- [[_COMMUNITY_Community 48|Community 48]]
- [[_COMMUNITY_Community 49|Community 49]]
- [[_COMMUNITY_Community 50|Community 50]]
- [[_COMMUNITY_Community 52|Community 52]]
- [[_COMMUNITY_Community 53|Community 53]]
- [[_COMMUNITY_Community 59|Community 59]]
- [[_COMMUNITY_Community 61|Community 61]]

## God Nodes (most connected - your core abstractions)
1. `@/layouts/AdminLayout.astro` - 64 edges
2. `@/lib/notification.svelte` - 39 edges
3. `tryGrpcCall()` - 38 edges
4. `@/components/admin/PermissionConfig.svelte` - 33 edges
5. `@/components/admin/UserList.svelte` - 30 edges
6. `@/components/admin/ProductForm.svelte` - 27 edges
7. `@/components/admin/InventoryList.svelte` - 26 edges
8. `@/components/admin/WarehouseList.svelte` - 26 edges
9. `@/components/admin/DepartmentList.svelte` - 24 edges
10. `@/components/admin/BomAddStep2.svelte` - 22 edges

## Surprising Connections (you probably didn't know these)
- `BomNodeWithProduct` --references--> `Product`  [EXTRACTED]
  lib/bom-node-utils.ts → types/api.ts
- `BomTreeNodeData` --references--> `BomNode`  [EXTRACTED]
  lib/bom-node-utils.ts → types/api.ts
- `getOrCreate()` --calls--> `createNotificationState()`  [EXTRACTED]
  lib/notification.svelte.ts → stores/index.svelte.ts
- `NodeData` --references--> `NodeType`  [EXTRACTED]
  components/admin/workflow/graph/serializer.ts → components/admin/workflow/graph/nodes.ts
- `GraphNode` --references--> `NodeType`  [EXTRACTED]
  components/admin/workflow/graph/serializer.ts → components/admin/workflow/graph/nodes.ts

## Communities (65 total, 6 thin omitted)

### Community 0 - "BOM Actions & Cost"
Cohesion: 0.08
Nodes (27): bom, user, createAt, page, pageSize, createAt, page, pageSize (+19 more)

### Community 1 - "SSR Data Loading"
Cohesion: 0.11
Nodes (21): logs, @/lib/grpc-client, @/lib/ssr-helpers, role, roleId, page, warehouseId, warehouseInfo (+13 more)

### Community 2 - "Warehouse & Location UI"
Cohesion: 0.11
Nodes (26): trimmedCode, trimmedName, @/components/admin/AdminProviders.svelte, ./LocationForm.svelte, ./WarehouseForm.svelte, ./TaskActionDialog.svelte, @/components/admin/workflow/WorkflowHistoryList.svelte, @/components/admin/workflow/WorkflowInstanceDetail.svelte (+18 more)

### Community 3 - "Audit Log UI"
Cohesion: 0.07
Nodes (23): auditStats, columns, d, filteredLogs, isLoading, todayMs, todayStart, batchActions (+15 more)

### Community 4 - "Workflow Graph Engine"
Cohesion: 0.09
Nodes (30): WorkflowEdgeData, getNodeTypeLabel(), isValidNodeType(), NODE_DEFS, NODE_TYPES, NodeDef, NodeType, capitalize() (+22 more)

### Community 5 - "Admin List Components"
Cohesion: 0.10
Nodes (22): start, endIndex, start, startIndex, totalPages, start, charCode, kw (+14 more)

### Community 6 - "Navigation & Permissions"
Cohesion: 0.13
Nodes (18): getAllMenuItems(), itemHasPermission(), MenuItem, ActionKey, isValidPermission(), PermissionCode, ResourceKey, toPermissionCode() (+10 more)

### Community 7 - "Workflow Pages"
Cohesion: 0.12
Nodes (21): @/lib/require-permission, @/lib/workflow-mappers, redirect, id, redirect, id, redirect, id (+13 more)

### Community 8 - "BOM Cost & Issue UI"
Cohesion: 0.12
Nodes (20): controller, date, start, date, result, searchLower, start, timeA (+12 more)

### Community 9 - "Permission Configuration"
Cohesion: 0.07
Nodes (14): allResources, avatarColors, changedPermissionCount, configRate, handleSave(), hasChanges, mobileSelectedRole, mobileShowSwitchConfirm (+6 more)

### Community 10 - "UI Component Library"
Cohesion: 0.15
Nodes (6): @/components/ui/MobileHeader.svelte, @/components/ui/MobileTabBar.svelte, @/lib/menu-items, @/lib/utils, svelte/elements, kw

### Community 11 - "Workflow Actions"
Cohesion: 0.11
Nodes (22): entityTypeSchema, instanceStatusSchema, taskStatusSchema, templateStatusSchema, workflow, Badge, ENTITY_TYPE_LABELS, ENTITY_TYPE_OPTIONS (+14 more)

### Community 12 - "Core Actions Registry"
Cohesion: 0.11
Nodes (14): audit, bomCategory, department, server, notification, permission, routing, routingStepSchema (+6 more)

### Community 13 - "Price History UI"
Cohesion: 0.09
Nodes (18): errorMessage, handleSubmit(), historyTotal, isLoadingHistory, isSubmitting, loadPriceHistory(), newPrice, remark (+10 more)

### Community 14 - "Inventory Management UI"
Cohesion: 0.12
Nodes (12): filtered, location, start, warehouseId, params, qty, @/components/admin/Inventory/InventoryLogList.svelte, @/components/admin/InventoryList.svelte (+4 more)

### Community 15 - "BOM Drag & Drop"
Cohesion: 0.10
Nodes (18): @atlaskit/pragmatic-drag-and-drop/element/adapter, childIds, children, edge, ids, matches, nameLower, totalPages (+10 more)

### Community 16 - "Product Form UI"
Cohesion: 0.10
Nodes (20): acquireChannel, categorySearch, categorySelectOptions, cateId, error, filteredCategoryOptions, generateCode(), handleSubmit() (+12 more)

### Community 17 - "BOM Type Definitions"
Cohesion: 0.10
Nodes (20): Bom, BOM_STATUS, BomDetail, BomLeafNode, EntityType, InstanceStatus, InventoryDetail, InventoryLog (+12 more)

### Community 18 - "Product Pages"
Cohesion: 0.12
Nodes (13): categoryOptions, redirect, termService, from, categoryOptions, copyFrom, redirect, page (+5 more)

### Community 19 - "User Management UI"
Cohesion: 0.13
Nodes (16): avatarGradients, batchActions, batchAssignRoles(), batchDelete(), columns, confirmDelete(), fetchRoles(), fetchUsers() (+8 more)

### Community 20 - "Workflow Visual Editor"
Cohesion: 0.17
Nodes (14): ./FlowCanvas.svelte, ./ApprovalConfig.svelte, ./AutoTaskConfig.svelte, ./ConditionBuilder.svelte, ./EdgeConfigPanel.svelte, ./JoinConfig.svelte, ./WorkflowGraphEditor.svelte, ./graph/edges (+6 more)

### Community 21 - "Layout & Navigation"
Cohesion: 0.13
Nodes (8): @/components/admin/Footer.svelte, @/components/ui/PageHeader.astro, id, @/layouts/AdminLayout.astro, string, bomId, from, from

### Community 22 - "Authentication"
Cohesion: 0.16
Nodes (11): auth, ExtendedLoginResponse, @buf/xweichen_abt.bufbuild_es/abt/v1/auth_pb, DEV_ADMIN_CREDENTIALS, hasSuperAdminRole, SessionData, @/lib/session, authService (+3 more)

### Community 23 - "Inventory Type Definitions"
Cohesion: 0.12
Nodes (16): BatchStockItem, BatchStockParams, InventoryItem, InventoryListParams, InventoryListResponse, InventoryLog, InventoryLogListResponse, InventoryLogParams (+8 more)

### Community 24 - "Dashboard & Sidebar"
Cohesion: 0.17
Nodes (10): hasActiveChild, @/components/admin/DashboardInventoryStats.svelte, @/components/admin/Header.svelte, @/components/admin/LowStockAlert.svelte, @/components/admin/Sidebar.svelte, ./ChangePasswordModal.svelte, @/lib/icons, @/lib/usePermission.svelte (+2 more)

### Community 25 - "Inventory & Location Actions"
Cohesion: 0.13
Nodes (11): inventory, location, from, inventoryPromises, leafProductIds, number, parentIds, productIds (+3 more)

### Community 26 - "Product & Role Actions"
Cohesion: 0.18
Nodes (11): product, role, syncH3yun, getAuthToken(), getTokenContext(), priceService, productService, roleService (+3 more)

### Community 27 - "BOM View & Cascade Inventory"
Cohesion: 0.14
Nodes (12): isWatched, next, isParent, parentIds, status, isWatched, next, start (+4 more)

### Community 28 - "User Edit Page"
Cohesion: 0.13
Nodes (11): departmentId, departments, user, userId, departmentService, from, from, from (+3 more)

### Community 29 - "Core Type Definitions"
Cohesion: 0.13
Nodes (14): ApiResponse, AuditLogInfo, Bom, BomNode, DepartmentInfo, PaginatedResponse, PermissionInfo, Product (+6 more)

### Community 30 - "Community 30"
Cohesion: 0.21
Nodes (7): ./ImportPageLayout.svelte, @/components/admin/sync/LaborImport.svelte, @/components/admin/sync/LocationImport.svelte, @/components/admin/sync/ProductImport.svelte, ./types, ../styles/global.css, @unocss/reset/tailwind-compat.css

### Community 31 - "Community 31"
Cohesion: 0.19
Nodes (7): sync, excelService, laborProcessService, @bufbuild/protobuf, collectStreamToResponse(), GET(), VALID_EXPORT_TYPES

### Community 32 - "Community 32"
Cohesion: 0.18
Nodes (7): error, handleSave(), hasChanges, isLoading, isSaving, loadData(), selectedResourcePermissions

### Community 33 - "Community 33"
Cohesion: 0.17
Nodes (6): filteredWarehouses, handleSuccess(), reloadData(), showDeleted, warehouseDrawerOpen, @/components/admin/WarehouseList.svelte

### Community 34 - "Community 34"
Cohesion: 0.23
Nodes (7): BomNodeWithProduct, BomTreeNodeData, getNodeRowStyle(), hasChildren(), isTopLevel(), BomNode, Product

### Community 35 - "Community 35"
Cohesion: 0.18
Nodes (10): a, date, match, result, searchLower, start, timeA, timeB (+2 more)

### Community 36 - "Community 36"
Cohesion: 0.24
Nodes (8): closeDrawer(), handleSubmit(), isInitialStockIn, loadInventory(), showDrawer, totalQuantity, validateForm(), @/components/admin/ProductWarehouseList.svelte

### Community 37 - "Community 37"
Cohesion: 0.20
Nodes (8): toAddDepts, toAddRoles, toRemoveDepts, toRemoveRoles, trimmedDisplayName, trimmedPassword, trimmedUsername, @/components/admin/UserForm.svelte

### Community 38 - "Community 38"
Cohesion: 0.36
Nodes (6): labor, handleApiError(), BusinessErrorDetails, FieldViolation, formatBusinessMessage(), parseErrorDetails()

### Community 39 - "Community 39"
Cohesion: 0.22
Nodes (6): dictCreateSchema, dictDeleteSchema, dictListSchema, dictUpdateSchema, laborDict, laborProcessDictService

### Community 40 - "Community 40"
Cohesion: 0.33
Nodes (6): createNotificationContext(), getOrCreate(), NotificationState, useNotification(), createNotificationState(), Notification

### Community 41 - "Community 41"
Cohesion: 0.28
Nodes (8): Location, Warehouse, CreateLocationInput, CreateWarehouseInput, UpdateLocationInput, UpdateWarehouseInput, WarehouseStatus, WarehouseWithLocations

### Community 42 - "Community 42"
Cohesion: 0.29
Nodes (6): filteredOptions, optionsLoading, parentSearch, selectedParentName, showDropdown, @/components/admin/TermForm.svelte

### Community 44 - "Community 44"
Cohesion: 0.40
Nodes (3): formatAmount(), formatCurrency(), FormatCurrencyOptions

### Community 45 - "Community 45"
Cohesion: 0.47
Nodes (3): IconPathData, iconPaths, StrokeIconData

### Community 46 - "Community 46"
Cohesion: 0.33
Nodes (5): @/components/admin/workflow/WorkflowInstanceList.svelte, badge, entityTypeFilter, isLoading, statusFilter

### Community 47 - "Community 47"
Cohesion: 0.40
Nodes (5): existing, hasEmpty, hasInvalid, items, @/components/admin/BomLaborCostPage.svelte

### Community 49 - "Community 49"
Cohesion: 0.50
Nodes (3): ImportMeta, ImportMetaEnv, Locals

### Community 50 - "Community 50"
Cohesion: 0.50
Nodes (3): API_PATHS, PAGINATION, TAXONOMY

## Knowledge Gaps
- **365 isolated node(s):** `ImportMetaEnv`, `ImportMeta`, `Locals`, `ExtendedLoginResponse`, `server` (+360 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **6 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `@/layouts/AdminLayout.astro` connect `Layout & Navigation` to `BOM Actions & Cost`, `SSR Data Loading`, `Warehouse & Location UI`, `Admin List Components`, `Community 37`, `Workflow Pages`, `UI Component Library`, `Product Pages`, `Community 52`, `Dashboard & Sidebar`, `Inventory & Location Actions`, `User Edit Page`, `Community 30`?**
  _High betweenness centrality (0.060) - this node is a cross-community bridge._
- **Why does `@/components/admin/PermissionConfig.svelte` connect `Permission Configuration` to `Dashboard & Sidebar`, `SSR Data Loading`, `Warehouse & Location UI`, `UI Component Library`?**
  _High betweenness centrality (0.050) - this node is a cross-community bridge._
- **Why does `@/components/admin/ProductForm.svelte` connect `Product Form UI` to `BOM Cost & Issue UI`, `Dashboard & Sidebar`, `Warehouse & Location UI`, `Product Pages`?**
  _High betweenness centrality (0.041) - this node is a cross-community bridge._
- **What connects `ImportMetaEnv`, `ImportMeta`, `Locals` to the rest of the system?**
  _365 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `BOM Actions & Cost` be split into smaller, more focused modules?**
  _Cohesion score 0.07965860597439545 - nodes in this community are weakly interconnected._
- **Should `SSR Data Loading` be split into smaller, more focused modules?**
  _Cohesion score 0.10810810810810811 - nodes in this community are weakly interconnected._
- **Should `Warehouse & Location UI` be split into smaller, more focused modules?**
  _Cohesion score 0.10695187165775401 - nodes in this community are weakly interconnected._