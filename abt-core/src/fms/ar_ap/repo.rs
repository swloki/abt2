use rust_decimal::Decimal;

use super::model::*;
use crate::fms::enums::CounterpartyType;
use crate::shared::enums::document_type::DocumentType;
use crate::shared::types::{DomainError, PageParams, PgExecutor, Result};

pub(crate) const LEDGER_COLUMNS: &str = "id, party_type, party_id, source_type, source_id, source_doc_no, \
    against_type, against_id, direction, amount, amount_applied, currency, exchange_rate, \
    transaction_date, due_date, period, description, operator_id, created_at";

const SETTLEMENT_COLUMNS: &str = "id, payment_source_type, payment_source_id, invoice_source_type, \
    invoice_source_id, amount, payment_ledger_id, invoice_ledger_id, exchange_gain_loss, \
    settlement_date, operator_id, created_at";

// ---------------------------------------------------------------------------
// ArApLedgerRepo
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 筛选条件构造（query_with_party / summary / query_details 三处共用）
// ---------------------------------------------------------------------------

/// 筛选参数值（异构类型，由 bind_filter! 按类型绑定到 $N 占位符）
pub(crate) enum FilterArg {
    PartyType(CounterpartyType),
    PartyId(i64),
    Text(String),
    Date(chrono::NaiveDate),
}

/// 按 build_filter_conditions 返回的 params 顺序绑定到 sqlx query
macro_rules! bind_filter {
    ($q:expr, $params:expr) => {{
        let mut q = $q;
        for p in $params {
            q = match p {
                FilterArg::PartyType(v) => q.bind(*v),
                FilterArg::PartyId(v) => q.bind(*v),
                FilterArg::Text(v) => q.bind(v.as_str()),
                FilterArg::Date(v) => q.bind(*v),
            };
        }
        q
    }};
}

/// 产品字段（product_code / pdt_name）EXISTS 子查询：匹配三种来源的行项目产品
fn product_field_cond(field: &str, n: usize) -> String {
    format!(
        "EXISTS(\
         SELECT 1 FROM shipping_request_items sri JOIN products p ON p.product_id = sri.product_id \
           WHERE sri.shipping_request_id = l.source_id AND l.source_type = 3 AND p.{field} ILIKE ${n} \
         UNION ALL SELECT 1 FROM purchase_order_items poi JOIN products p ON p.product_id = poi.product_id \
           WHERE poi.order_id = l.source_id AND l.source_type = 7 AND p.{field} ILIKE ${n} \
         UNION ALL SELECT 1 FROM outsourcing_orders oo JOIN products p ON p.product_id = oo.product_id \
           WHERE oo.id = l.source_id AND l.source_type = 11 AND p.{field} ILIKE ${n})",
        field = field,
        n = n
    )
}

/// 销售经理/采购员（users.display_name）EXISTS：销售发货→销售经理、采购入库→采购员
fn rep_cond(n: usize) -> String {
    format!(
        "EXISTS(\
         SELECT 1 FROM shipping_requests sr JOIN sales_orders so ON so.id = sr.order_id \
           JOIN users u ON u.user_id = so.sales_rep_id \
           WHERE sr.id = l.source_id AND l.source_type = 3 AND u.display_name ILIKE ${n} \
         UNION ALL SELECT 1 FROM purchase_orders po JOIN users u ON u.user_id = po.operator_id \
           WHERE po.id = l.source_id AND l.source_type = 7 AND u.display_name ILIKE ${n})",
        n = n
    )
}

/// 由 ArApLedgerFilter 构造 WHERE 条件片段与绑定参数。
/// 条件 SQL 中参数编号 ${n} 按绑定顺序 1,2,3...；调用方用 bind_filter! 绑定 params，
/// LIMIT/OFFSET 或其它附加参数编号从 params.len()+1 起。
pub(crate) fn build_filter_conditions(
    filter: &ArApLedgerFilter,
) -> (Vec<String>, Vec<FilterArg>) {
    let mut conds: Vec<String> = Vec::new();
    let mut params: Vec<FilterArg> = Vec::new();

    if let Some(pt) = filter.party_type {
        params.push(FilterArg::PartyType(pt));
        conds.push(format!("l.party_type = ${}", params.len()));
    }
    if let Some(pid) = filter.party_id {
        params.push(FilterArg::PartyId(pid));
        conds.push(format!("l.party_id = ${}", params.len()));
    }
    if filter.outstanding_only {
        conds.push("l.amount - l.amount_applied > 0".into());
    }
    if let Some(ref kw) = filter.keyword {
        let t = kw.trim();
        if !t.is_empty() {
            params.push(FilterArg::Text(format!("%{t}%")));
            let n = params.len();
            conds.push(format!(
                "(EXISTS(SELECT 1 FROM customers c WHERE c.customer_id = l.party_id AND c.customer_name ILIKE ${n}) \
                  OR EXISTS(SELECT 1 FROM suppliers s WHERE s.supplier_id = l.party_id AND s.supplier_name ILIKE ${n}))"
            ));
        }
    }
    if let Some(ref d) = filter.doc_no {
        let t = d.trim();
        if !t.is_empty() {
            params.push(FilterArg::Text(format!("%{t}%")));
            conds.push(format!("l.source_doc_no ILIKE ${}", params.len()));
        }
    }
    if let Some(ref code) = filter.product_code {
        let t = code.trim();
        if !t.is_empty() {
            params.push(FilterArg::Text(format!("%{t}%")));
            conds.push(product_field_cond("product_code", params.len()));
        }
    }
    if let Some(ref name) = filter.product_name {
        let t = name.trim();
        if !t.is_empty() {
            params.push(FilterArg::Text(format!("%{t}%")));
            conds.push(product_field_cond("pdt_name", params.len()));
        }
    }
    if let Some(ref rep) = filter.rep_name {
        let t = rep.trim();
        if !t.is_empty() {
            params.push(FilterArg::Text(format!("%{t}%")));
            conds.push(rep_cond(params.len()));
        }
    }
    if let Some(ref p) = filter.period {
        let t = p.trim();
        if !t.is_empty() {
            params.push(FilterArg::Text(t.to_string()));
            conds.push(format!("l.period = ${}", params.len()));
        }
    }
    if let Some(d) = filter.start_date {
        params.push(FilterArg::Date(d));
        conds.push(format!("l.transaction_date >= ${}", params.len()));
    }
    if let Some(d) = filter.end_date {
        params.push(FilterArg::Date(d));
        conds.push(format!("l.transaction_date <= ${}", params.len()));
    }

    (conds, params)
}

pub struct ArApLedgerRepo;

impl ArApLedgerRepo {
    /// 插入一条台账记录。原子幂等：`(source_type, source_id)` 冲突时 DO NOTHING 返回 None
    ///（依赖 migration 072 全局唯一索引 ar_ap_ledger_source_uniq）。
    /// 返回 `Some(id)` 表示新建，`None` 表示已存在（冲突跳过；调用方可改调 rewrite_amount_by_source 重算金额）。
    pub async fn insert(executor: PgExecutor<'_>, row: &ArApLedgerInsert<'_>) -> Result<Option<i64>> {
        let id: Option<i64> = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO ar_ap_ledger
               (party_type, party_id, source_type, source_id, source_doc_no,
                against_type, against_id, direction, amount, currency, exchange_rate,
                transaction_date, due_date, period, description, operator_id)
               VALUES ($1,$2,$3,$4,$5,$6, $7,$8,$9,$10,$11,$12, $13,$14,$15,$16)
               ON CONFLICT (source_type, source_id) DO NOTHING
               RETURNING id"#,
        )
        .bind(row.party_type)
        .bind(row.party_id)
        .bind(row.source_type)
        .bind(row.source_id)
        .bind(row.source_doc_no)
        .bind(row.against_type)
        .bind(row.against_id)
        .bind(row.direction)
        .bind(row.amount)
        .bind(row.currency)
        .bind(row.exchange_rate)
        .bind(row.transaction_date)
        .bind(row.due_date)
        .bind(row.period)
        .bind(row.description)
        .bind(row.operator_id)
        .fetch_optional(executor)
        .await?;
        Ok(id)
    }

    /// 按 source 重写台账金额（PO 维度多次部分收货重算应付用）。
    /// 仅 amount_applied=0（未核销）允许改；已核销或行不存在报错（业务规则，避免污染已对账数据）。
    pub async fn rewrite_amount_by_source(
        executor: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
        new_amount: Decimal,
    ) -> Result<()> {
        let affected = sqlx::query(
            "UPDATE ar_ap_ledger SET amount = $3, updated_at = NOW() \
             WHERE source_type = $1 AND source_id = $2 AND amount_applied = 0",
        )
        .bind(source_type)
        .bind(source_id)
        .bind(new_amount)
        .execute(&mut *executor)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(DomainError::business_rule(format!(
                "台账 source_type={:?} source_id={} 已核销或不存在，不可重写金额",
                source_type, source_id
            )));
        }
        Ok(())
    }

    /// 查往来方币种（customers/suppliers.currency，缺省 CNY）。与 adjustment/cash_journal 口径一致。
    pub async fn fetch_party_currency(
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<String> {
        let sql = match party_type {
            CounterpartyType::Customer => {
                "SELECT currency FROM customers WHERE customer_id = $1 AND deleted_at IS NULL"
            }
            CounterpartyType::Supplier => {
                "SELECT currency FROM suppliers WHERE supplier_id = $1 AND deleted_at IS NULL"
            }
            _ => return Ok("CNY".to_string()),
        };
        let currency = sqlx::query_scalar::<sqlx::Postgres, Option<String>>(sql)
            .bind(party_id)
            .fetch_optional(&mut *db)
            .await?
            .flatten()
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "CNY".to_string());
        Ok(currency)
    }

    /// 幂等插入反向台账行（退货/冲减用）：`source_type + source_id` 已存在则跳过（返回 `None`），
    /// 否则按往来方币种 insert（返回新 ledger id）。`against` 留空，由业务层按需补。
    pub async fn insert_reversal_if_absent(
        db: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
        source_type: DocumentType,
        source_id: i64,
        source_doc_no: &str,
        direction: super::enums::LedgerDirection,
        amount: Decimal,
        description: &str,
        operator_id: i64,
    ) -> Result<Option<i64>> {
        // 幂等由 ArApLedgerRepo::insert 的 ON CONFLICT 兜底（partial unique index，migration 070）
        let currency = Self::fetch_party_currency(db, party_type, party_id).await?;
        let period = chrono::Utc::now().format("%Y-%m").to_string();
        let today = chrono::Local::now().date_naive();
        let id = ArApLedgerRepo::insert(
            db,
            &ArApLedgerInsert {
                party_type,
                party_id,
                source_type,
                source_id,
                source_doc_no,
                against_type: None,
                against_id: None,
                direction,
                amount,
                currency: &currency,
                exchange_rate: Decimal::ONE,
                transaction_date: today,
                due_date: None,
                period: &period,
                description,
                operator_id,
            },
        )
        .await?;
        Ok(id)
    }

    /// 更新 amount_applied（核销/反核销时调用）
    pub async fn update_amount_applied(
        executor: PgExecutor<'_>,
        id: i64,
        new_applied: Decimal,
    ) -> Result<u64> {
        let result = sqlx::query::<sqlx::Postgres>(
            r#"UPDATE ar_ap_ledger
               SET amount_applied = $1
               WHERE id = $2"#,
        )
        .bind(new_applied)
        .bind(id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 按 id 查询单条记录
    pub async fn get_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<ArApLedger>> {
        let row = sqlx::query_as::<sqlx::Postgres, ArApLedger>(sqlx::AssertSqlSafe(format!(
            "SELECT {LEDGER_COLUMNS} FROM ar_ap_ledger WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    /// 按 source_type + source_id 查询未清台账记录
    pub async fn get_open_by_source(
        executor: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<Option<ArApLedger>> {
        let row = sqlx::query_as::<sqlx::Postgres, ArApLedger>(sqlx::AssertSqlSafe(format!(
            "SELECT {LEDGER_COLUMNS} FROM ar_ap_ledger \
             WHERE source_type = $1 AND source_id = $2 AND amount - amount_applied > 0 \
             LIMIT 1"
        )))
        .bind(source_type)
        .bind(source_id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    /// 分页查询台账（含 JOIN party name 和 account info）
    pub async fn query_with_party(
        executor: PgExecutor<'_>,
        filter: &ArApLedgerFilter,
        page: &PageParams,
    ) -> Result<(Vec<ArApLedgerRow>, u64)> {
        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx: u32 = 0;

        let pt_param: Option<CounterpartyType> = if let Some(pt) = filter.party_type {
            param_idx += 1;
            conditions.push(format!("l.party_type = ${}", param_idx));
            Some(pt)
        } else { None };

        let pid_param: Option<i64> = if let Some(pid) = filter.party_id {
            param_idx += 1;
            conditions.push(format!("l.party_id = ${}", param_idx));
            Some(pid)
        } else { None };

        if filter.outstanding_only {
            conditions.push("l.amount - l.amount_applied > 0".to_string());
        }

        let kw_param: Option<String> = if let Some(ref kw) = filter.keyword {
            let t = kw.trim();
            if !t.is_empty() {
                param_idx += 1; let n = param_idx;
                conditions.push(format!("(EXISTS(SELECT 1 FROM customers c WHERE c.customer_id=l.party_id AND c.customer_name ILIKE ${n}) OR EXISTS(SELECT 1 FROM suppliers s WHERE s.supplier_id=l.party_id AND s.supplier_name ILIKE ${n}))"));
                Some(format!("%{t}%"))
            } else { None }
        } else { None };

        let doc_param: Option<String> = if let Some(ref d) = filter.doc_no {
            let t = d.trim();
            if !t.is_empty() { param_idx += 1; conditions.push(format!("l.source_doc_no ILIKE ${}", param_idx)); Some(format!("%{t}%")) }
            else { None }
        } else { None };
        let pcode_param: Option<String> = if let Some(ref c) = filter.product_code {
            let t = c.trim();
            if !t.is_empty() { param_idx += 1; let n = param_idx as usize; conditions.push(product_field_cond("product_code", n)); Some(format!("%{t}%")) }
            else { None }
        } else { None };
        let pname_param: Option<String> = if let Some(ref n) = filter.product_name {
            let t = n.trim();
            if !t.is_empty() { param_idx += 1; let m = param_idx as usize; conditions.push(product_field_cond("pdt_name", m)); Some(format!("%{t}%")) }
            else { None }
        } else { None };
        let rep_param: Option<String> = if let Some(ref r) = filter.rep_name {
            let t = r.trim();
            if !t.is_empty() { param_idx += 1; let n = param_idx as usize; conditions.push(rep_cond(n)); Some(format!("%{t}%")) }
            else { None }
        } else { None };
        let per_param: Option<String> = if let Some(ref p) = filter.period {
            if !p.trim().is_empty() { param_idx += 1; conditions.push(format!("l.period = ${}", param_idx)); Some(p.clone()) } else { None }
        } else { None };

        let start_param: Option<chrono::NaiveDate> = if let Some(d) = filter.start_date {
            param_idx += 1; conditions.push(format!("l.transaction_date >= ${}", param_idx)); Some(d)
        } else { None };
        let end_param: Option<chrono::NaiveDate> = if let Some(d) = filter.end_date {
            param_idx += 1; conditions.push(format!("l.transaction_date <= ${}", param_idx)); Some(d)
        } else { None };

        let where_clause = if conditions.is_empty() { String::new() } else { format!("WHERE {}", conditions.join(" AND ")) };

        let count_sql = format!("SELECT COUNT(*) FROM ar_ap_ledger l {w}", w = where_clause);
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(pt) = pt_param { count_q = count_q.bind(pt); }
        if let Some(pid) = pid_param { count_q = count_q.bind(pid); }
        if let Some(ref k) = kw_param { count_q = count_q.bind(k); }
        if let Some(ref d) = doc_param { count_q = count_q.bind(d); }
        if let Some(ref c) = pcode_param { count_q = count_q.bind(c); }
        if let Some(ref n) = pname_param { count_q = count_q.bind(n); }
        if let Some(ref r) = rep_param { count_q = count_q.bind(r); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1; let limit_idx = param_idx;
        param_idx += 1; let offset_idx = param_idx;

        let data_sql = format!(
            r#"SELECT l.id, l.party_type, l.party_id,
                      COALESCE(c.customer_name, s.supplier_name, 'Unknown') AS party_name,
                      l.source_type, l.source_id, l.source_doc_no,
                      l.direction, l.amount, l.amount_applied,
                      l.amount - l.amount_applied AS amount_outstanding,
                      l.currency, l.transaction_date, l.due_date, l.period, l.description,
                      COALESCE(
                        (SELECT po.doc_number FROM purchase_orders po
                         WHERE po.id = l.source_id AND l.source_type = 7 AND po.deleted_at IS NULL),
                        (SELECT so.doc_number FROM shipping_requests sr
                         JOIN sales_orders so ON so.id = sr.order_id AND so.deleted_at IS NULL
                         WHERE sr.id = l.source_id AND l.source_type = 3)
                      ) AS upstream_doc_no,
                      COALESCE(
                        (SELECT string_agg(DISTINCT p.pdt_name, '、')
                         FROM purchase_order_items poi
                         JOIN products p ON p.product_id = poi.product_id
                         WHERE poi.order_id = l.source_id AND l.source_type = 7 AND poi.received_qty > 0),
                        (SELECT p.pdt_name FROM outsourcing_orders oo
                         JOIN products p ON p.product_id = oo.product_id
                         WHERE oo.id = l.source_id AND l.source_type = 11),
                        (SELECT string_agg(DISTINCT p.pdt_name, '、')
                         FROM shipping_request_items sri
                         JOIN products p ON p.product_id = sri.product_id
                         WHERE sri.shipping_request_id = l.source_id AND l.source_type = 3)
                      ) AS product_summary
               FROM ar_ap_ledger l
               LEFT JOIN customers c ON l.party_type = 1 AND c.customer_id = l.party_id AND c.deleted_at IS NULL
               LEFT JOIN suppliers s ON l.party_type = 2 AND s.supplier_id = l.party_id AND s.deleted_at IS NULL
               {w}
               ORDER BY l.transaction_date DESC, l.id DESC
               LIMIT ${li} OFFSET ${oi}"#,
            w = where_clause,
            li = limit_idx,
            oi = offset_idx,
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, ArApLedgerRow>(sqlx::AssertSqlSafe(data_sql));
        if let Some(pt) = pt_param { data_q = data_q.bind(pt); }
        if let Some(pid) = pid_param { data_q = data_q.bind(pid); }
        if let Some(ref k) = kw_param { data_q = data_q.bind(k); }
        if let Some(ref d) = doc_param { data_q = data_q.bind(d); }
        if let Some(ref c) = pcode_param { data_q = data_q.bind(c); }
        if let Some(ref n) = pname_param { data_q = data_q.bind(n); }
        if let Some(ref r) = rep_param { data_q = data_q.bind(r); }
        if let Some(ref p) = per_param { data_q = data_q.bind(p); }
        if let Some(d) = start_param { data_q = data_q.bind(d); }
        if let Some(d) = end_param { data_q = data_q.bind(d); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;
        Ok((items, total))
    }

    /// 查询台账明细（产品行项目级，导出明细表用，不分页）
    ///
    /// 用 CTE 先按 filter 筛出 ar_ap_ledger 行 + 往来方名称，再 UNION ALL 三种来源
    /// 的行项目级明细：采购入库(多产品/PO单价)、委外(单产品/主表单价)、销售发货(多产品/SO单价)。
    pub async fn query_details(
        executor: PgExecutor<'_>,
        filter: &ArApLedgerFilter,
    ) -> Result<Vec<ArApLedgerDetailRow>> {
        let (conds, params) = build_filter_conditions(filter);
        let where_clause = if conds.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conds.join(" AND "))
        };

        let sql = format!(
            r#"WITH filtered AS (
                SELECT l.id, l.source_type, l.source_id, l.source_doc_no, l.transaction_date, l.amount,
                       COALESCE(c.customer_name, s.supplier_name, 'Unknown') AS party_name
                FROM ar_ap_ledger l
                LEFT JOIN customers c ON l.party_type = 1 AND c.customer_id = l.party_id AND c.deleted_at IS NULL
                LEFT JOIN suppliers s ON l.party_type = 2 AND s.supplier_id = l.party_id AND s.deleted_at IS NULL
                {where_clause}
              )
              SELECT f.id, f.party_name, f.source_doc_no,
                     po.doc_number, 7::SMALLINT,
                     p.product_code, p.pdt_name AS product_name,
                     poi.received_qty AS quantity,
                     COALESCE(poi.unit_price, 0) AS unit_price,
                     COALESCE(poi.received_qty * poi.unit_price, 0) AS line_amount,
                     f.transaction_date
              FROM filtered f
              JOIN purchase_orders po ON po.id = f.source_id AND f.source_type = 7
              JOIN purchase_order_items poi ON poi.order_id = po.id AND poi.received_qty > 0
              JOIN products p ON p.product_id = poi.product_id
              UNION ALL
              SELECT f.id, f.party_name, f.source_doc_no,
                     NULL, 11::SMALLINT,
                     p.product_code, p.pdt_name,
                     COALESCE(f.amount / NULLIF(oo.unit_price, 0), 0) AS quantity,
                     oo.unit_price AS unit_price,
                     f.amount AS line_amount,
                     f.transaction_date
              FROM filtered f
              JOIN outsourcing_orders oo ON oo.id = f.source_id AND f.source_type = 11
              JOIN products p ON p.product_id = oo.product_id
              UNION ALL
              SELECT f.id, f.party_name, f.source_doc_no,
                     so.doc_number, 3::SMALLINT,
                     p.product_code, p.pdt_name,
                     sri.shipped_qty AS quantity,
                     COALESCE(soi.unit_price, 0) AS unit_price,
                     COALESCE(sri.shipped_qty * soi.unit_price, 0) AS line_amount,
                     f.transaction_date
              FROM filtered f
              JOIN shipping_requests sr ON sr.id = f.source_id AND f.source_type = 3
              LEFT JOIN sales_orders so ON so.id = sr.order_id
              JOIN shipping_request_items sri ON sri.shipping_request_id = sr.id AND sri.shipped_qty > 0
              LEFT JOIN sales_order_items soi ON soi.id = sri.order_item_id
              JOIN products p ON p.product_id = sri.product_id
              ORDER BY transaction_date DESC, ledger_id"#,
            where_clause = where_clause,
        );

        let q = sqlx::query_as::<sqlx::Postgres, ArApLedgerDetailRow>(sqlx::AssertSqlSafe(sql));
        let q = bind_filter!(q, &params);
        let items = q.fetch_all(executor).await?;
        Ok(items)
    }

    /// 台账汇总（按 filter 聚合：总额/未清/逾期/7天内到期，逾期基准=due_date）
    pub async fn summary(
        executor: PgExecutor<'_>,
        filter: &ArApLedgerFilter,
        today: chrono::NaiveDate,
    ) -> Result<LedgerSummary> {
        let (conds, params) = build_filter_conditions(filter);
        let where_clause = if conds.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conds.join(" AND "))
        };

        // today / today+7 用于 SELECT 里的逾期与 7 天内到期 CASE（编号紧随 filter 参数之后）
        let today_idx = params.len() + 1;
        let today7_idx = params.len() + 2;
        let today7 = today + chrono::Duration::days(7);


        let sql = format!(
            r#"SELECT
                COALESCE(SUM(l.amount), 0) AS total_amount,
                COALESCE(SUM(l.amount - l.amount_applied), 0) AS total_outstanding,
                COALESCE(SUM(CASE WHEN l.due_date IS NOT NULL AND l.due_date < ${t}
                                  AND l.amount - l.amount_applied > 0
                             THEN l.amount - l.amount_applied ELSE 0 END), 0) AS total_overdue,
                COALESCE(SUM(CASE WHEN l.due_date IS NOT NULL AND l.due_date BETWEEN ${t} AND ${t7}
                                  AND l.amount - l.amount_applied > 0
                             THEN l.amount - l.amount_applied ELSE 0 END), 0) AS due_within_7d
               FROM ar_ap_ledger l {w}"#,
            t = today_idx,
            t7 = today7_idx,
            w = where_clause,
        );

        let q = sqlx::query_as::<sqlx::Postgres, LedgerSummary>(sqlx::AssertSqlSafe(sql));
        let q = bind_filter!(q, &params);
        let q = q.bind(today).bind(today7);

        let summary = q.fetch_one(executor).await?;
        Ok(summary)
    }

    /// 按 id 查询单条台账行（含 party_name/upstream/product_summary，drawer 详情用）
    pub async fn get_detail_row(
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<ArApLedgerRow>> {
        let row = sqlx::query_as::<sqlx::Postgres, ArApLedgerRow>(sqlx::AssertSqlSafe(format!(
            r#"SELECT l.id, l.party_type, l.party_id,
                      COALESCE(c.customer_name, s.supplier_name, 'Unknown') AS party_name,
                      l.source_type, l.source_id, l.source_doc_no,
                      l.direction, l.amount, l.amount_applied,
                      l.amount - l.amount_applied AS amount_outstanding,
                      l.currency, l.transaction_date, l.due_date, l.period, l.description,
                      COALESCE(
                        (SELECT po.doc_number FROM purchase_orders po
                         WHERE po.id = l.source_id AND l.source_type = 7 AND po.deleted_at IS NULL),
                        (SELECT so.doc_number FROM shipping_requests sr
                         JOIN sales_orders so ON so.id = sr.order_id AND so.deleted_at IS NULL
                         WHERE sr.id = l.source_id AND l.source_type = 3)
                      ) AS upstream_doc_no,
                      COALESCE(
                        (SELECT string_agg(DISTINCT p.pdt_name, '、')
                         FROM purchase_order_items poi JOIN products p ON p.product_id = poi.product_id
                         WHERE poi.order_id = l.source_id AND l.source_type = 7 AND poi.received_qty > 0),
                        (SELECT p.pdt_name FROM outsourcing_orders oo JOIN products p ON p.product_id = oo.product_id
                         WHERE oo.id = l.source_id AND l.source_type = 11),
                        (SELECT string_agg(DISTINCT p.pdt_name, '、')
                         FROM shipping_request_items sri JOIN products p ON p.product_id = sri.product_id
                         WHERE sri.shipping_request_id = l.source_id AND l.source_type = 3)
                      ) AS product_summary
               FROM ar_ap_ledger l
               LEFT JOIN customers c ON l.party_type = 1 AND c.customer_id = l.party_id AND c.deleted_at IS NULL
               LEFT JOIN suppliers s ON l.party_type = 2 AND s.supplier_id = l.party_id AND s.deleted_at IS NULL
               WHERE l.id = $1"#
        )))
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    /// 查询台账对应的产品行项目清单（drawer 详情用，按 source_type 分流）
    pub async fn get_detail_items(
        executor: PgExecutor<'_>,
        source_type: DocumentType,
        source_id: i64,
    ) -> Result<Vec<LedgerDetailItem>> {
        let sql = match source_type {
            DocumentType::PurchaseOrder => {
                // 采购入库 — PO 明细 × 产品 × 单价
                r#"SELECT p.product_code, p.pdt_name AS product_name,
                          poi.received_qty AS quantity,
                          COALESCE(poi.unit_price, 0) AS unit_price,
                          COALESCE(poi.received_qty * poi.unit_price, 0) AS line_amount
                   FROM purchase_order_items poi
                   JOIN products p ON p.product_id = poi.product_id
                   WHERE poi.order_id = $1 AND poi.received_qty > 0"#
            }
            DocumentType::OutsourcingOrder => {
                // 委外 — 单产品（加工费）
                r#"SELECT p.product_code, p.pdt_name AS product_name,
                          oo.completed_qty AS quantity,
                          oo.unit_price,
                          oo.completed_qty * oo.unit_price AS line_amount
                   FROM outsourcing_orders oo
                   JOIN products p ON p.product_id = oo.product_id
                   WHERE oo.id = $1"#
            }
            _ => {
                // 销售发货 — 发货明细 × 产品 × SO 单价
                r#"SELECT p.product_code, p.pdt_name AS product_name,
                          sri.shipped_qty AS quantity,
                          COALESCE(soi.unit_price, 0) AS unit_price,
                          COALESCE(sri.shipped_qty * soi.unit_price, 0) AS line_amount
                   FROM shipping_request_items sri
                   JOIN products p ON p.product_id = sri.product_id
                   LEFT JOIN sales_order_items soi ON soi.id = sri.order_item_id
                   WHERE sri.shipping_request_id = $1 AND sri.shipped_qty > 0"#
            }
        };
        let rows = sqlx::query_as::<sqlx::Postgres, LedgerDetailItem>(sqlx::AssertSqlSafe(sql))
            .bind(source_id)
            .fetch_all(executor)
            .await?;
        Ok(rows)
    }

    /// 查询某往来方的未清发票（用于核销选择）
    pub async fn list_open_invoices(
        executor: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<Vec<OpenInvoice>> {
        let rows = sqlx::query_as::<sqlx::Postgres, OpenInvoice>(
            r#"SELECT l.source_type, l.source_id, l.source_doc_no AS doc_number,
                      l.transaction_date AS issue_date, l.due_date,
                      l.amount AS total,
                      l.amount - l.amount_applied AS outstanding,
                      l.currency
               FROM ar_ap_ledger l
               WHERE l.party_type = $1 AND l.party_id = $2
                 AND l.amount - l.amount_applied > 0
                 AND l.direction = CASE WHEN $1 = 1 THEN 1 ELSE 2 END
               ORDER BY l.due_date ASC NULLS LAST, l.id ASC"#,
        )
        .bind(party_type)
        .bind(party_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 查询未分配的收款/付款（用于核销选择）
    pub async fn list_unapplied_payments(
        executor: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<Vec<UnappliedPayment>> {
        let rows = sqlx::query_as::<sqlx::Postgres, UnappliedPayment>(
            r#"SELECT l.source_type, l.source_id, l.source_doc_no AS doc_number,
                      l.transaction_date,
                      l.amount AS amount,
                      l.amount - l.amount_applied AS unapplied,
                      l.currency
               FROM ar_ap_ledger l
               WHERE l.party_type = $1 AND l.party_id = $2
                 AND l.amount - l.amount_applied > 0
                 AND l.direction = CASE WHEN $1 = 1 THEN 2 ELSE 1 END
                 AND l.source_type = 30
               ORDER BY l.transaction_date ASC, l.id ASC"#,
        )
        .bind(party_type)
        .bind(party_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 获取往来方余额（聚合查询）
    pub async fn get_party_balance(
        executor: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_id: i64,
    ) -> Result<PartyBalance> {
        let row = sqlx::query_as::<sqlx::Postgres, PartyBalance>(
            r#"SELECT
                  $1::SMALLINT AS party_type,
                  $2::BIGINT AS party_id,
                  COALESCE(
                    (SELECT c.customer_name FROM customers c WHERE c.customer_id = $2 AND c.deleted_at IS NULL),
                    (SELECT s.supplier_name FROM suppliers s WHERE s.supplier_id = $2 AND s.deleted_at IS NULL),
                    'Unknown'
                  ) AS party_name,
                  COALESCE(SUM(CASE WHEN l.direction = 1 THEN l.amount - l.amount_applied ELSE 0 END), 0) AS total_ar,
                  COALESCE(SUM(CASE WHEN l.direction = 2 THEN l.amount - l.amount_applied ELSE 0 END), 0) AS total_ap,
                  COALESCE(SUM(CASE WHEN l.direction = 1 THEN l.amount - l.amount_applied
                                     WHEN l.direction = 2 THEN -(l.amount - l.amount_applied)
                                     ELSE 0 END), 0) AS net_balance,
                  MAX(l.currency) AS currency
               FROM ar_ap_ledger l
               WHERE l.party_type = $1 AND l.party_id = $2
                 AND l.amount - l.amount_applied > 0"#,
        )
        .bind(party_type)
        .bind(party_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    /// 批量获取往来方余额
    pub async fn batch_party_balances(
        executor: PgExecutor<'_>,
        party_type: CounterpartyType,
        party_ids: &[i64],
    ) -> Result<Vec<PartyBalance>> {
        if party_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query_as::<sqlx::Postgres, PartyBalance>(
            r#"SELECT
                  l.party_type,
                  l.party_id,
                  COALESCE(
                    (SELECT c.customer_name FROM customers c WHERE c.customer_id = l.party_id AND c.deleted_at IS NULL),
                    (SELECT s.supplier_name FROM suppliers s WHERE s.supplier_id = l.party_id AND s.deleted_at IS NULL),
                    'Unknown'
                  ) AS party_name,
                  COALESCE(SUM(CASE WHEN l.direction = 1 THEN l.amount - l.amount_applied ELSE 0 END), 0) AS total_ar,
                  COALESCE(SUM(CASE WHEN l.direction = 2 THEN l.amount - l.amount_applied ELSE 0 END), 0) AS total_ap,
                  COALESCE(SUM(CASE WHEN l.direction = 1 THEN l.amount - l.amount_applied
                                     WHEN l.direction = 2 THEN -(l.amount - l.amount_applied)
                                     ELSE 0 END), 0) AS net_balance,
                  MAX(l.currency) AS currency
               FROM ar_ap_ledger l
               WHERE l.party_type = $1
                 AND l.party_id = ANY($2)
                 AND l.amount - l.amount_applied > 0
               GROUP BY l.party_type, l.party_id"#,
        )
        .bind(party_type)
        .bind(party_ids)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 账龄分析查询：获取未清台账记录按往来方聚合的账龄分布
    pub async fn aging_query(
        executor: PgExecutor<'_>,
        party_type: CounterpartyType,
        _as_of_date: chrono::NaiveDate,
        party_ids: Option<&[i64]>,
    ) -> Result<Vec<(i64, String, Decimal, Option<chrono::NaiveDate>)>> {
        // 返回 (party_id, party_name, outstanding, due_date) 以便在 Rust 侧计算账龄分段
        let rows = sqlx::query_as::<sqlx::Postgres, (i64, String, Decimal, Option<chrono::NaiveDate>)>(
            r#"SELECT
                  l.party_id,
                  COALESCE(
                    (SELECT c.customer_name FROM customers c WHERE c.customer_id = l.party_id AND c.deleted_at IS NULL),
                    (SELECT s.supplier_name FROM suppliers s WHERE s.supplier_id = l.party_id AND s.deleted_at IS NULL),
                    'Unknown'
                  ) AS party_name,
                  l.amount - l.amount_applied AS outstanding,
                  l.due_date
               FROM ar_ap_ledger l
               WHERE l.party_type = $1
                 AND l.amount - l.amount_applied > 0
                 AND ($2::BIGINT[] IS NULL OR l.party_id = ANY($2))
               ORDER BY l.party_id, l.due_date ASC NULLS LAST"#,
        )
        .bind(party_type)
        .bind(party_ids.map(|ids| ids.to_vec()))
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}

/// 用于插入的简化参数结构
#[derive(Debug, Clone)]
pub struct ArApLedgerInsert<'a> {
    pub party_type: CounterpartyType,
    pub party_id: i64,
    pub source_type: DocumentType,
    pub source_id: i64,
    pub source_doc_no: &'a str,
    pub against_type: Option<DocumentType>,
    pub against_id: Option<i64>,
    pub direction: super::enums::LedgerDirection,
    pub amount: Decimal,
    pub currency: &'a str,
    pub exchange_rate: Decimal,
    pub transaction_date: chrono::NaiveDate,
    pub due_date: Option<chrono::NaiveDate>,
    pub period: &'a str,
    pub description: &'a str,
    pub operator_id: i64,
}

// ---------------------------------------------------------------------------
// ArApSettlementRepo
// ---------------------------------------------------------------------------

pub struct ArApSettlementRepo;

impl ArApSettlementRepo {
    /// 创建核销记录
    pub async fn insert(
        executor: PgExecutor<'_>,
        payment_source_type: DocumentType,
        payment_source_id: i64,
        invoice_source_type: DocumentType,
        invoice_source_id: i64,
        amount: Decimal,
        payment_ledger_id: i64,
        invoice_ledger_id: i64,
        exchange_gain_loss: Decimal,
        settlement_date: chrono::NaiveDate,
        operator_id: i64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO ar_ap_settlements
               (payment_source_type, payment_source_id, invoice_source_type, invoice_source_id,
                amount, payment_ledger_id, invoice_ledger_id, exchange_gain_loss, settlement_date, operator_id)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
               RETURNING id"#,
        )
        .bind(payment_source_type)
        .bind(payment_source_id)
        .bind(invoice_source_type)
        .bind(invoice_source_id)
        .bind(amount)
        .bind(payment_ledger_id)
        .bind(invoice_ledger_id)
        .bind(exchange_gain_loss)
        .bind(settlement_date)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 删除核销记录（反核销）
    pub async fn delete(executor: PgExecutor<'_>, id: i64) -> Result<u64> {
        let result = sqlx::query::<sqlx::Postgres>(
            "DELETE FROM ar_ap_settlements WHERE id = $1",
        )
        .bind(id)
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    /// 按 id 查询
    pub async fn get_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<ArApSettlement>> {
        let row = sqlx::query_as::<sqlx::Postgres, ArApSettlement>(sqlx::AssertSqlSafe(format!(
            "SELECT {SETTLEMENT_COLUMNS} FROM ar_ap_settlements WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    /// 查询某发票的所有核销记录
    pub async fn list_by_invoice(
        executor: PgExecutor<'_>,
        invoice_source_type: DocumentType,
        invoice_source_id: i64,
    ) -> Result<Vec<ArApSettlement>> {
        let rows = sqlx::query_as::<sqlx::Postgres, ArApSettlement>(sqlx::AssertSqlSafe(format!(
            "SELECT {SETTLEMENT_COLUMNS} FROM ar_ap_settlements
             WHERE invoice_source_type = $1 AND invoice_source_id = $2
             ORDER BY settlement_date DESC"
        )))
        .bind(invoice_source_type)
        .bind(invoice_source_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 查询某付款的所有核销记录
    pub async fn list_by_payment(
        executor: PgExecutor<'_>,
        payment_source_type: DocumentType,
        payment_source_id: i64,
    ) -> Result<Vec<ArApSettlement>> {
        let rows = sqlx::query_as::<sqlx::Postgres, ArApSettlement>(sqlx::AssertSqlSafe(format!(
            "SELECT {SETTLEMENT_COLUMNS} FROM ar_ap_settlements
             WHERE payment_source_type = $1 AND payment_source_id = $2
             ORDER BY settlement_date DESC"
        )))
        .bind(payment_source_type)
        .bind(payment_source_id)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 分页查询核销记录
    pub async fn query(
        executor: PgExecutor<'_>,
        filter: &SettlementFilter,
        page: &PageParams,
    ) -> Result<(Vec<ArApSettlement>, u64)> {
        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx: u32 = 0;

        if let Some(t) = filter.payment_source_type {
            param_idx += 1;
            conditions.push(format!("payment_source_type = ${}", param_idx));
            let _ = t;
        }
        // Simplified: we'll build conditions manually for known filters
        // Full implementation would match the pattern above

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let count_sql = format!("SELECT COUNT(*) FROM ar_ap_settlements {where_clause}");
        let count_q =
            sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        // Reset and bind...
        // For now, simpler approach:
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {SETTLEMENT_COLUMNS} FROM ar_ap_settlements {where_clause} \
             ORDER BY settlement_date DESC, id DESC LIMIT ${} OFFSET ${}",
            limit_idx, offset_idx
        );
        let data_q =
            sqlx::query_as::<sqlx::Postgres, ArApSettlement>(sqlx::AssertSqlSafe(data_sql));
        let items = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64)
            .fetch_all(executor)
            .await?;

        Ok((items, total))
    }

    /// 计算某发票已核销总额
    pub async fn sum_settled_by_invoice(
        executor: PgExecutor<'_>,
        invoice_source_type: DocumentType,
        invoice_source_id: i64,
    ) -> Result<Decimal> {
        let total: Option<Decimal> = sqlx::query_scalar::<sqlx::Postgres, Decimal>(
            r#"SELECT COALESCE(SUM(amount), 0)
               FROM ar_ap_settlements
               WHERE invoice_source_type = $1 AND invoice_source_id = $2"#,
        )
        .bind(invoice_source_type)
        .bind(invoice_source_id)
        .fetch_optional(executor)
        .await?;
        Ok(total.unwrap_or(Decimal::ZERO))
    }
}
