//! 分类数据模型
//!
//! 包含分类实体及其元数据结构。

use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};

/// 分类实体
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Term {
    pub term_id: i64,
    pub term_name: String,
    pub term_parent: i64,
    /// 分类法类型（如 "category", "warehouse" 等）
    pub taxonomy: String,
    pub term_meta: TermMeta,
}

impl<'r> FromRow<'r, PgRow> for Term {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let term_id: i64 = row.try_get("term_id")?;
        let term_name: String = row.try_get("term_name")?;
        let term_parent: i64 = row.try_get("term_parent")?;
        let taxonomy: String = row.try_get("taxonomy")?;
        let term_meta_value: serde_json::Value = row.try_get("term_meta")?;
        let term_meta: TermMeta =
            serde_json::from_value(term_meta_value).map_err(|e| sqlx::Error::ColumnDecode {
                index: "term_meta".to_string(),
                source: Box::new(e),
            })?;
        Ok(Term {
            term_id,
            term_name,
            term_parent,
            taxonomy,
            term_meta,
        })
    }
}

/// 分类元数据（存储在 JSONB 字段中）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TermMeta {
    /// 关联的产品数量
    pub count: i64,
}

/// 分类与产品的关联关系
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TermRelation {
    pub term_id: i64,
    pub product_id: i64,
}

// ============================================================================
// 查询参数
// ============================================================================

/// 分类查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TermQuery {
    /// 分类名称（模糊匹配）
    pub term_name: Option<String>,
    /// 父分类 ID
    pub term_parent: Option<i64>,
    /// 分类法类型
    pub taxonomy: Option<String>,
}

// ============================================================================
// 树形结构
// ============================================================================

/// 带子节点的分类树
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TermTree {
    pub term_id: i64,
    pub term_name: String,
    pub term_parent: i64,
    pub taxonomy: String,
    pub term_meta: TermMeta,
    /// 子分类
    pub children: Vec<TermTree>,
}

impl From<Term> for TermTree {
    fn from(term: Term) -> Self {
        Self {
            term_id: term.term_id,
            term_name: term.term_name,
            term_parent: term.term_parent,
            taxonomy: term.taxonomy,
            term_meta: term.term_meta,
            children: Vec::new(),
        }
    }
}

// ============================================================================
// 创建/更新请求
// ============================================================================

/// 创建分类请求
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTermRequest {
    /// 分类名称
    pub term_name: String,
    /// 父分类 ID（0 表示顶级分类）
    pub term_parent: i64,
    /// 分类法类型
    pub taxonomy: String,
}

/// 更新分类请求
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTermRequest {
    /// 分类名称
    pub term_name: String,
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_term_default() {
        let term = Term::default();
        assert_eq!(term.term_id, 0);
        assert_eq!(term.term_name, "");
        assert_eq!(term.term_parent, 0);
        assert_eq!(term.taxonomy, "");
        assert_eq!(term.term_meta.count, 0);
    }

    #[test]
    fn test_term_meta_default() {
        let meta = TermMeta::default();
        assert_eq!(meta.count, 0);
    }

    #[test]
    fn test_term_serialization() {
        let term = Term {
            term_id: 1,
            term_name: "电子产品".to_string(),
            term_parent: 0,
            taxonomy: "category".to_string(),
            term_meta: TermMeta { count: 10 },
        };

        let json = serde_json::to_string(&term).unwrap();
        let deserialized: Term = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.term_id, 1);
        assert_eq!(deserialized.term_name, "电子产品");
        assert_eq!(deserialized.term_parent, 0);
        assert_eq!(deserialized.taxonomy, "category");
        assert_eq!(deserialized.term_meta.count, 10);
    }

    #[test]
    fn test_term_query_default() {
        let query = TermQuery::default();
        assert!(query.term_name.is_none());
        assert!(query.term_parent.is_none());
        assert!(query.taxonomy.is_none());
    }

    #[test]
    fn test_term_tree_from_term() {
        let term = Term {
            term_id: 1,
            term_name: "根分类".to_string(),
            term_parent: 0,
            taxonomy: "category".to_string(),
            term_meta: TermMeta { count: 5 },
        };

        let tree: TermTree = term.into();
        assert_eq!(tree.term_id, 1);
        assert_eq!(tree.term_name, "根分类");
        assert_eq!(tree.term_parent, 0);
        assert_eq!(tree.taxonomy, "category");
        assert_eq!(tree.term_meta.count, 5);
        assert!(tree.children.is_empty());
    }

    #[test]
    fn test_create_term_request() {
        let request = CreateTermRequest {
            term_name: "新分类".to_string(),
            term_parent: 1,
            taxonomy: "category".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: CreateTermRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.term_name, "新分类");
        assert_eq!(deserialized.term_parent, 1);
        assert_eq!(deserialized.taxonomy, "category");
    }

    #[test]
    fn test_update_term_request() {
        let request = UpdateTermRequest {
            term_name: "更新后的分类名".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: UpdateTermRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.term_name, "更新后的分类名");
    }

    #[test]
    fn test_term_relation() {
        let relation = TermRelation {
            term_id: 1,
            product_id: 100,
        };

        let json = serde_json::to_string(&relation).unwrap();
        let deserialized: TermRelation = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.term_id, 1);
        assert_eq!(deserialized.product_id, 100);
    }

    #[test]
    fn test_term_tree_with_children() {
        let tree = TermTree {
            term_id: 1,
            term_name: "父分类".to_string(),
            term_parent: 0,
            taxonomy: "category".to_string(),
            term_meta: TermMeta { count: 2 },
            children: vec![
                TermTree {
                    term_id: 2,
                    term_name: "子分类1".to_string(),
                    term_parent: 1,
                    taxonomy: "category".to_string(),
                    term_meta: TermMeta { count: 1 },
                    children: vec![],
                },
                TermTree {
                    term_id: 3,
                    term_name: "子分类2".to_string(),
                    term_parent: 1,
                    taxonomy: "category".to_string(),
                    term_meta: TermMeta { count: 1 },
                    children: vec![],
                },
            ],
        };

        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.children[0].term_name, "子分类1");
        assert_eq!(tree.children[1].term_name, "子分类2");
    }
}
