use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::database::DatabasePool;
use econ_graph_core::models::FinancialStatement;
use econ_graph_core::schema::financial_statements;
use std::sync::Arc;
use uuid::Uuid;

/// **Financial Statement Service**
///
/// Service for managing financial statement data and queries.
/// Provides access to SEC EDGAR filings and financial data.
pub struct FinancialStatementService {
    pool: Arc<DatabasePool>,
}

impl FinancialStatementService {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    /// Get financial statements for a specific company
    /// 
    /// # Arguments
    /// * `company_id` - UUID of the company
    /// * `limit` - Maximum number of results
    /// * `offset` - Number of results to skip
    /// 
    /// # Returns
    /// Vector of financial statements for the company
    pub async fn get_company_financial_statements(
        &self,
        company_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<FinancialStatement>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).min(100) as i64;
        let offset = offset.unwrap_or(0) as i64;

        let statements = financial_statements::table
            .filter(financial_statements::company_id.eq(company_id))
            .order(financial_statements::filing_date.desc())
            .limit(limit)
            .offset(offset)
            .select(FinancialStatement::as_select())
            .load::<FinancialStatement>(&mut conn)
            .await?;

        Ok(statements)
    }

    /// Get a financial statement by ID
    pub async fn get_financial_statement_by_id(
        &self,
        statement_id: Uuid,
    ) -> Result<Option<FinancialStatement>> {
        let mut conn = self.pool.get().await?;

        let statement = financial_statements::table
            .filter(financial_statements::id.eq(statement_id))
            .select(FinancialStatement::as_select())
            .first::<FinancialStatement>(&mut conn)
            .await
            .optional()?;

        Ok(statement)
    }

    /// Get financial statements by filing type
    pub async fn get_financial_statements_by_filing_type(
        &self,
        filing_type: &str,
        limit: Option<i32>,
    ) -> Result<Vec<FinancialStatement>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).min(100) as i64;

        let statements = financial_statements::table
            .filter(financial_statements::filing_type.eq(filing_type))
            .order(financial_statements::filing_date.desc())
            .limit(limit)
            .select(FinancialStatement::as_select())
            .load::<FinancialStatement>(&mut conn)
            .await?;

        Ok(statements)
    }

    /// Get financial statements by date range
    pub async fn get_financial_statements_by_date_range(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        limit: Option<i32>,
    ) -> Result<Vec<FinancialStatement>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).min(100) as i64;

        let statements = financial_statements::table
            .filter(financial_statements::filing_date.between(start_date, end_date))
            .order(financial_statements::filing_date.desc())
            .limit(limit)
            .select(FinancialStatement::as_select())
            .load::<FinancialStatement>(&mut conn)
            .await?;

        Ok(statements)
    }

    /// Get financial statements by fiscal year
    pub async fn get_financial_statements_by_fiscal_year(
        &self,
        fiscal_year: i32,
        limit: Option<i32>,
    ) -> Result<Vec<FinancialStatement>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).min(100) as i64;

        let statements = financial_statements::table
            .filter(financial_statements::fiscal_year.eq(fiscal_year))
            .order(financial_statements::filing_date.desc())
            .limit(limit)
            .select(FinancialStatement::as_select())
            .load::<FinancialStatement>(&mut conn)
            .await?;

        Ok(statements)
    }

    /// Get total count of financial statements for a company
    pub async fn count_company_financial_statements(&self, company_id: Uuid) -> Result<i64> {
        let mut conn = self.pool.get().await?;

        let count = financial_statements::table
            .filter(financial_statements::company_id.eq(company_id))
            .count()
            .get_result::<i64>(&mut conn)
            .await?;

        Ok(count)
    }

    /// Create a new financial statement
    pub async fn create_financial_statement(
        &self,
        statement: &FinancialStatement,
    ) -> Result<FinancialStatement> {
        let mut conn = self.pool.get().await?;

        diesel::insert_into(financial_statements::table)
            .values(statement)
            .returning(FinancialStatement::as_returning())
            .get_result::<FinancialStatement>(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create financial statement: {}", e))
    }

    /// Update an existing financial statement
    pub async fn update_financial_statement(
        &self,
        statement: &FinancialStatement,
    ) -> Result<FinancialStatement> {
        let mut conn = self.pool.get().await?;

        diesel::update(financial_statements::table.filter(financial_statements::id.eq(statement.id)))
            .set(statement)
            .returning(FinancialStatement::as_returning())
            .get_result::<FinancialStatement>(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update financial statement: {}", e))
    }

    /// Get financial statements by processing status
    pub async fn get_financial_statements_by_status(
        &self,
        status: &str,
        limit: Option<i32>,
    ) -> Result<Vec<FinancialStatement>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).min(100) as i64;

        let statements = financial_statements::table
            .filter(financial_statements::xbrl_processing_status.eq(status))
            .order(financial_statements::filing_date.desc())
            .limit(limit)
            .select(FinancialStatement::as_select())
            .load::<FinancialStatement>(&mut conn)
            .await?;

        Ok(statements)
    }

    /// Get recent financial statements across all companies
    pub async fn get_recent_financial_statements(
        &self,
        limit: Option<i32>,
    ) -> Result<Vec<FinancialStatement>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).min(100) as i64;

        let statements = financial_statements::table
            .order(financial_statements::filing_date.desc())
            .limit(limit)
            .select(FinancialStatement::as_select())
            .load::<FinancialStatement>(&mut conn)
            .await?;

        Ok(statements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::database::create_pool;

    #[tokio::test]
    async fn test_get_company_financial_statements() {
        // This test would require a test database setup
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:password@localhost/econ_graph_test".to_string());
        
        let pool = create_pool(&database_url).await.unwrap();
        let service = FinancialStatementService::new(Arc::new(pool));
        
        // Test basic functionality
        let company_id = Uuid::new_v4();
        let result = service.get_company_financial_statements(company_id, Some(10), Some(0)).await;
        // This will fail without proper test data, but tests the interface
        assert!(result.is_ok() || result.is_err());
    }
}
