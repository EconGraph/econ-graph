use anyhow::Result;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::database::DatabasePool;
use econ_graph_core::models::Company;
use econ_graph_core::schema::companies;
use std::sync::Arc;
use uuid::Uuid;

pub struct CompanyService {
    pool: Arc<DatabasePool>,
}

impl CompanyService {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    pub async fn search_companies(
        &self,
        query: &str,
        limit: Option<i32>,
        include_inactive: Option<bool>,
    ) -> Result<Vec<Company>> {
        let mut conn = self.pool.get().await?;
        let limit = limit.unwrap_or(50).clamp(1, 100) as i64;
        let include_inactive = include_inactive.unwrap_or(false);

        // Match the company name, ticker, CIK, and legal name case-insensitively.
        let search_query = format!("%{}%", query);

        let mut diesel_query = companies::table
            .filter(
                companies::name
                    .ilike(&search_query)
                    .or(companies::ticker.ilike(&search_query))
                    .or(companies::cik.ilike(&search_query))
                    .or(companies::legal_name.ilike(&search_query)),
            )
            .order(companies::name.asc())
            .limit(limit)
            .into_boxed();

        // Filter out inactive companies unless explicitly requested
        if !include_inactive {
            diesel_query = diesel_query.filter(companies::is_active.eq(true));
        }

        let results = diesel_query
            .select(Company::as_select())
            .load::<Company>(&mut *conn)
            .await?;

        Ok(results)
    }

    pub async fn get_company_by_id(&self, company_id: Uuid) -> Result<Option<Company>> {
        let mut conn = self.pool.get().await?;

        let company = companies::table
            .filter(companies::id.eq(company_id))
            .select(Company::as_select())
            .first::<Company>(&mut *conn)
            .await
            .optional()?;

        Ok(company)
    }

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
                companies::name
                    .ilike(&search_query)
                    .or(companies::ticker.ilike(&search_query))
                    .or(companies::cik.ilike(&search_query))
                    .or(companies::legal_name.ilike(&search_query)),
            )
            .into_boxed();

        if !include_inactive {
            diesel_query = diesel_query.filter(companies::is_active.eq(true));
        }

        let count = diesel_query.count().get_result::<i64>(&mut *conn).await?;

        Ok(count)
    }
}
