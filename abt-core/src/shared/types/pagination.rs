/// 数据范围 — 行级数据权限
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataScope {
    /// 全部数据（管理员）
    All,
    /// 本部门（经理）
    Department,
    /// 仅本人（业务员）
    SelfOnly,
}

/// 分页查询参数
#[derive(Debug, Clone)]
pub struct PageParams {
    pub page: u32,
    pub page_size: u32,
}

impl PageParams {
    pub fn new(page: u32, page_size: u32) -> Self {
        Self {
            page: page.max(1),
            page_size: page_size.clamp(1, 200),
        }
    }

    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.page_size
    }
}

/// 统一分页返回结构
#[derive(Debug, Clone)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total: u64, page: u32, page_size: u32) -> Self {
        let total_pages = if page_size == 0 {
            0
        } else {
            (total as u32).div_ceil(page_size)
        };
        Self {
            items,
            total,
            page,
            page_size,
            total_pages,
        }
    }

    pub fn empty(page: u32, page_size: u32) -> Self {
        Self {
            items: vec![],
            total: 0,
            page,
            page_size,
            total_pages: 0,
        }
    }
}
