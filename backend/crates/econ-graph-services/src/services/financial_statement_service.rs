use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::database::DatabasePool;
use econ_graph_core::models::FinancialStatement;
use econ_graph_core::schema::financial_statements;
use std::sync::Arc;
use uuid::Uuid;

pub struct FinancialStatementService {
    pool: Arc<DatabasePool>,
}

impl FinancialStatementService {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    pub async fn get_company_financial_statements(
        &self,
        company_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<FinancialStatement>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).clamp(1, 100) as i64;
        let offset = offset.unwrap_or(0) as i64;

        let statements = financial_statements::table
            .filter(financial_statements::company_id.eq(company_id))
            .order(financial_statements::filing_date.desc())
            .limit(limit)
            .offset(offset)
            .select(FinancialStatement::as_select())
            .load::<FinancialStatement>(&mut *conn)
            .await?;

        Ok(statements)
    }

    pub async fn count_company_financial_statements(&self, company_id: Uuid) -> Result<i64> {
        let mut conn = self.pool.get().await?;

        let count = financial_statements::table
            .filter(financial_statements::company_id.eq(company_id))
            .count()
            .get_result::<i64>(&mut *conn)
            .await?;

        Ok(count)
    }
}
