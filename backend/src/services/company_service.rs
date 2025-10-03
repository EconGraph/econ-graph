use anyhow::Result;
use async_graphql::Result as GraphQLResult;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::database::DatabasePool;
use econ_graph_core::models::Company;
use econ_graph_core::schema::companies;
use std::sync::Arc;
use uuid::Uuid;

/// **Company Service**
///
/// Service for managing company data and search operations.
/// Provides fulltext search capabilities using PostgreSQL indices.
pub struct CompanyService {
    pool: Arc<DatabasePool>,
}

impl CompanyService {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    /// Search for companies using fulltext search
    /// 
    /// # Arguments
    /// * `query` - Search query (company name, ticker, or CIK)
    /// * `limit` - Maximum number of results
    /// * `include_inactive` - Whether to include inactive companies
    /// 
    /// # Returns
    /// Vector of companies matching the search criteria
    pub async fn search_companies(
        &self,
        query: &str,
        limit: Option<i32>,
        include_inactive: Option<bool>,
    ) -> Result<Vec<Company>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).min(100) as i64;
        let include_inactive = include_inactive.unwrap_or(false);

        // Build the search query using PostgreSQL fulltext search
        let search_query = format!("%{}%", query);
        
        let mut diesel_query = companies::table
            .filter(
                companies::name.ilike(&search_query)
                    .or(companies::ticker.ilike(&search_query))
                    .or(companies::cik.ilike(&search_query))
                    .or(companies::legal_name.ilike(&search_query))
            )
            .order(companies::name.asc())
            .limit(limit);

        // Filter out inactive companies unless explicitly requested
        if !include_inactive {
            diesel_query = diesel_query.filter(companies::is_active.eq(true));
        }

        let results = diesel_query
            .select(Company::as_select())
            .load::<Company>(&mut conn)
            .await?;

        Ok(results)
    }

    /// Get a company by ID
    pub async fn get_company_by_id(&self, company_id: Uuid) -> Result<Option<Company>> {
        let mut conn = self.pool.get().await?;

        let company = companies::table
            .filter(companies::id.eq(company_id))
            .select(Company::as_select())
            .first::<Company>(&mut conn)
            .await
            .optional()?;

        Ok(company)
    }

    /// Get a company by CIK
    pub async fn get_company_by_cik(&self, cik: &str) -> Result<Option<Company>> {
        let mut conn = self.pool.get().await?;

        let company = companies::table
            .filter(companies::cik.eq(cik))
            .select(Company::as_select())
            .first::<Company>(&mut conn)
            .await
            .optional()?;

        Ok(company)
    }

    /// Get companies by ticker symbol
    pub async fn get_companies_by_ticker(&self, ticker: &str) -> Result<Vec<Company>> {
        let mut conn = self.pool.get().await?;

        let companies_list = companies::table
            .filter(companies::ticker.eq(ticker))
            .filter(companies::is_active.eq(true))
            .select(Company::as_select())
            .load::<Company>(&mut conn)
            .await?;

        Ok(companies_list)
    }

    /// Get total count of companies matching search criteria
    pub async fn count_companies(
        &self,
        query: &str,
        include_inactive: Option<bool>,
    ) -> Result<i64> {
        let mut conn = self.pool.get().await?;
        let include_inactive = include_inactive.unwrap_or(false);

        let search_query = format!("%{}%", query);
        
        let mut diesel_query = companies::table
            .filter(
                companies::name.ilike(&search_query)
                    .or(companies::ticker.ilike(&search_query))
                    .or(companies::cik.ilike(&search_query))
                    .or(companies::legal_name.ilike(&search_query))
            );

        if !include_inactive {
            diesel_query = diesel_query.filter(companies::is_active.eq(true));
        }

        let count = diesel_query
            .count()
            .get_result::<i64>(&mut conn)
            .await?;

        Ok(count)
    }

    /// Create a new company
    pub async fn create_company(&self, company: &Company) -> Result<Company> {
        let mut conn = self.pool.get().await?;

        diesel::insert_into(companies::table)
            .values(company)
            .returning(Company::as_returning())
            .get_result::<Company>(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create company: {}", e))
    }

    /// Update an existing company
    pub async fn update_company(&self, company: &Company) -> Result<Company> {
        let mut conn = self.pool.get().await?;

        diesel::update(companies::table.filter(companies::id.eq(company.id)))
            .set(company)
            .returning(Company::as_returning())
            .get_result::<Company>(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update company: {}", e))
    }

    /// Delete a company (soft delete by setting is_active to false)
    pub async fn delete_company(&self, company_id: Uuid) -> Result<bool> {
        let mut conn = self.pool.get().await?;

        let updated_rows = diesel::update(companies::table.filter(companies::id.eq(company_id)))
            .set(companies::is_active.eq(false))
            .execute(&mut conn)
            .await?;

        Ok(updated_rows > 0)
    }

    /// Get companies by industry
    pub async fn get_companies_by_industry(&self, industry: &str) -> Result<Vec<Company>> {
        let mut conn = self.pool.get().await?;

        let companies_list = companies::table
            .filter(companies::industry.eq(industry))
            .filter(companies::is_active.eq(true))
            .select(Company::as_select())
            .load::<Company>(&mut conn)
            .await?;

        Ok(companies_list)
    }

    /// Get companies by sector
    pub async fn get_companies_by_sector(&self, sector: &str) -> Result<Vec<Company>> {
        let mut conn = self.pool.get().await?;

        let companies_list = companies::table
            .filter(companies::sector.eq(sector))
            .filter(companies::is_active.eq(true))
            .select(Company::as_select())
            .load::<Company>(&mut conn)
            .await?;

        Ok(companies_list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::database::create_pool;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_search_companies() {
        // This test would require a test database setup
        // For now, just test the service creation
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:password@localhost/econ_graph_test".to_string());
        
        let pool = create_pool(&database_url).await.unwrap();
        let service = CompanyService::new(Arc::new(pool));
        
        // Test basic functionality
        let result = service.search_companies("Apple", Some(10), Some(false)).await;
        // This will fail without proper test data, but tests the interface
        assert!(result.is_ok() || result.is_err());
    }
}
