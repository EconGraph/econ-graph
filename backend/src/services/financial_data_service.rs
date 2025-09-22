use crate::models::{
    Company, FinancialStatement, FinancialLineItem, FinancialAnnotation,
    AnnotationReply, AnnotationAssignment, AnnotationTemplate,
    NewCompany, NewFinancialStatement, NewFinancialLineItem,
    NewFinancialAnnotation, NewAnnotationReply, NewAnnotationAssignment, NewAnnotationTemplate,
    AnnotationFilter, AnnotationReplyFilter, AnnotationAssignmentFilter, AnnotationTemplateFilter,
};
use crate::schema::{
    companies, financial_statements, financial_line_items, financial_annotations,
    annotation_replies, annotation_assignments, annotation_templates,
};
use diesel::prelude::*;
use diesel::PgConnection;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Service for managing financial data operations
pub struct FinancialDataService<'a> {
    conn: &'a PgConnection,
}

impl<'a> FinancialDataService<'a> {
    pub fn new(conn: &'a PgConnection) -> Self {
        Self { conn }
    }

    // Company operations
    pub fn create_company(&self, new_company: NewCompany) -> Result<Company, diesel::result::Error> {
        diesel::insert_into(companies::table)
            .values(&new_company)
            .get_result(self.conn)
    }

    pub fn get_company_by_cik(&self, cik: &str) -> Result<Option<Company>, diesel::result::Error> {
        companies::table
            .filter(companies::cik.eq(cik))
            .first(self.conn)
            .optional()
    }

    pub fn get_company_by_ticker(&self, ticker: &str) -> Result<Option<Company>, diesel::result::Error> {
        companies::table
            .filter(companies::ticker.eq(ticker))
            .first(self.conn)
            .optional()
    }

    pub fn search_companies(&self, query: &str, limit: i64) -> Result<Vec<Company>, diesel::result::Error> {
        companies::table
            .filter(
                companies::name.ilike(format!("%{}%", query))
                    .or(companies::ticker.ilike(format!("%{}%", query)))
                    .or(companies::legal_name.ilike(format!("%{}%", query)))
            )
            .limit(limit)
            .load(self.conn)
    }

    // Financial statement operations
    pub fn create_financial_statement(&self, new_statement: NewFinancialStatement) -> Result<FinancialStatement, diesel::result::Error> {
        diesel::insert_into(financial_statements::table)
            .values(&new_statement)
            .get_result(self.conn)
    }

    pub fn get_financial_statements_by_company(&self, company_id: Uuid, limit: i64) -> Result<Vec<FinancialStatement>, diesel::result::Error> {
        financial_statements::table
            .filter(financial_statements::company_id.eq(company_id))
            .order(financial_statements::filing_date.desc())
            .limit(limit)
            .load(self.conn)
    }

    pub fn get_financial_statement_by_accession(&self, accession_number: &str) -> Result<Option<FinancialStatement>, diesel::result::Error> {
        financial_statements::table
            .filter(financial_statements::accession_number.eq(accession_number))
            .first(self.conn)
            .optional()
    }

    pub fn update_xbrl_processing_status(
        &self,
        statement_id: Uuid,
        status: &str,
        error_message: Option<&str>
    ) -> Result<FinancialStatement, diesel::result::Error> {
        diesel::update(financial_statements::table.filter(financial_statements::id.eq(statement_id)))
            .set((
                financial_statements::xbrl_processing_status.eq(status),
                financial_statements::xbrl_processing_error.eq(error_message),
                financial_statements::updated_at.eq(Utc::now()),
            ))
            .get_result(self.conn)
    }

    pub fn update_xbrl_file_info(
        &self,
        statement_id: Uuid,
        file_oid: Option<i32>,
        file_content: Option<Vec<u8>>,
        file_size: Option<i64>,
        file_hash: Option<&str>,
    ) -> Result<FinancialStatement, diesel::result::Error> {
        diesel::update(financial_statements::table.filter(financial_statements::id.eq(statement_id)))
            .set((
                financial_statements::xbrl_file_oid.eq(file_oid),
                financial_statements::xbrl_file_content.eq(file_content),
                financial_statements::xbrl_file_size_bytes.eq(file_size),
                financial_statements::xbrl_file_hash.eq(file_hash),
                financial_statements::xbrl_file_compressed.eq(true),
                financial_statements::xbrl_file_compression_type.eq(Some("zstd")),
                financial_statements::updated_at.eq(Utc::now()),
            ))
            .get_result(self.conn)
    }

    // Financial line item operations
    pub fn create_financial_line_item(&self, new_line_item: NewFinancialLineItem) -> Result<FinancialLineItem, diesel::result::Error> {
        diesel::insert_into(financial_line_items::table)
            .values(&new_line_item)
            .get_result(self.conn)
    }

    pub fn create_financial_line_items_batch(&self, line_items: Vec<NewFinancialLineItem>) -> Result<Vec<FinancialLineItem>, diesel::result::Error> {
        diesel::insert_into(financial_line_items::table)
            .values(&line_items)
            .get_results(self.conn)
    }

    pub fn get_financial_line_items_by_statement(&self, statement_id: Uuid) -> Result<Vec<FinancialLineItem>, diesel::result::Error> {
        financial_line_items::table
            .filter(financial_line_items::statement_id.eq(statement_id))
            .order(financial_line_items::statement_type.asc())
            .then_order_by(financial_line_items::level.asc())
            .then_order_by(financial_line_items::order_index.asc())
            .load(self.conn)
    }

    pub fn get_financial_line_items_by_concept(&self, concept: &str, limit: i64) -> Result<Vec<FinancialLineItem>, diesel::result::Error> {
        financial_line_items::table
            .filter(financial_line_items::taxonomy_concept.eq(concept))
            .order(financial_line_items::created_at.desc())
            .limit(limit)
            .load(self.conn)
    }

    // Annotation operations
    pub fn create_annotation(&self, new_annotation: NewFinancialAnnotation) -> Result<FinancialAnnotation, diesel::result::Error> {
        diesel::insert_into(financial_annotations::table)
            .values(&new_annotation)
            .get_result(self.conn)
    }

    pub fn get_annotations_by_filter(&self, filter: AnnotationFilter) -> Result<Vec<FinancialAnnotation>, diesel::result::Error> {
        let mut query = financial_annotations::table.into_boxed();

        if let Some(statement_id) = filter.statement_id {
            query = query.filter(financial_annotations::statement_id.eq(statement_id));
        }
        if let Some(line_item_id) = filter.line_item_id {
            query = query.filter(financial_annotations::line_item_id.eq(line_item_id));
        }
        if let Some(author_id) = filter.author_id {
            query = query.filter(financial_annotations::author_id.eq(author_id));
        }
        if let Some(annotation_type) = filter.annotation_type {
            query = query.filter(financial_annotations::annotation_type.eq(annotation_type.to_string()));
        }
        if let Some(status) = filter.status {
            query = query.filter(financial_annotations::status.eq(status.to_string()));
        }
        if let Some(is_private) = filter.is_private {
            query = query.filter(financial_annotations::is_private.eq(is_private));
        }
        if let Some(created_after) = filter.created_after {
            query = query.filter(financial_annotations::created_at.ge(created_after));
        }
        if let Some(created_before) = filter.created_before {
            query = query.filter(financial_annotations::created_at.le(created_before));
        }

        query
            .order(financial_annotations::created_at.desc())
            .load(self.conn)
    }

    pub fn create_annotation_reply(&self, new_reply: NewAnnotationReply) -> Result<AnnotationReply, diesel::result::Error> {
        diesel::insert_into(annotation_replies::table)
            .values(&new_reply)
            .get_result(self.conn)
    }

    pub fn get_annotation_replies(&self, annotation_id: Uuid) -> Result<Vec<AnnotationReply>, diesel::result::Error> {
        annotation_replies::table
            .filter(annotation_replies::annotation_id.eq(annotation_id))
            .order(annotation_replies::created_at.asc())
            .load(self.conn)
    }

    // Assignment operations
    pub fn create_assignment(&self, new_assignment: NewAnnotationAssignment) -> Result<AnnotationAssignment, diesel::result::Error> {
        diesel::insert_into(annotation_assignments::table)
            .values(&new_assignment)
            .get_result(self.conn)
    }

    pub fn get_assignments_by_filter(&self, filter: AnnotationAssignmentFilter) -> Result<Vec<AnnotationAssignment>, diesel::result::Error> {
        let mut query = annotation_assignments::table.into_boxed();

        if let Some(statement_id) = filter.statement_id {
            query = query.filter(annotation_assignments::statement_id.eq(statement_id));
        }
        if let Some(assignee_id) = filter.assignee_id {
            query = query.filter(annotation_assignments::assignee_id.eq(assignee_id));
        }
        if let Some(status) = filter.status {
            query = query.filter(annotation_assignments::status.eq(status.to_string()));
        }

        query
            .order(annotation_assignments::due_date.asc())
            .load(self.conn)
    }

    // Template operations
    pub fn create_template(&self, new_template: NewAnnotationTemplate) -> Result<AnnotationTemplate, diesel::result::Error> {
        diesel::insert_into(annotation_templates::table)
            .values(&new_template)
            .get_result(self.conn)
    }

    pub fn get_templates_by_filter(&self, filter: AnnotationTemplateFilter) -> Result<Vec<AnnotationTemplate>, diesel::result::Error> {
        let mut query = annotation_templates::table.into_boxed();

        if let Some(created_by) = filter.created_by {
            query = query.filter(annotation_templates::created_by.eq(created_by));
        }
        if let Some(annotation_type) = filter.annotation_type {
            query = query.filter(annotation_templates::annotation_type.eq(annotation_type.to_string()));
        }
        if let Some(is_public) = filter.is_public {
            query = query.filter(annotation_templates::is_public.eq(is_public));
        }
        if let Some(name_contains) = filter.name_contains {
            query = query.filter(annotation_templates::name.ilike(format!("%{}%", name_contains)));
        }

        query
            .order(annotation_templates::usage_count.desc())
            .then_order_by(annotation_templates::created_at.desc())
            .load(self.conn)
    }

    // Analytics and reporting operations
    pub fn get_company_financial_summary(&self, company_id: Uuid, periods: i32) -> Result<HashMap<String, Vec<f64>>, diesel::result::Error> {
        // This would implement complex financial analysis queries
        // For now, return empty HashMap as placeholder
        Ok(HashMap::new())
    }

    pub fn get_peer_comparison_data(&self, company_id: Uuid, industry: &str) -> Result<HashMap<String, f64>, diesel::result::Error> {
        // This would implement peer comparison queries
        // For now, return empty HashMap as placeholder
        Ok(HashMap::new())
    }

    pub fn calculate_financial_ratios(&self, statement_id: Uuid) -> Result<HashMap<String, f64>, diesel::result::Error> {
        // This would implement financial ratio calculations
        // For now, return empty HashMap as placeholder
        Ok(HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use uuid::Uuid;

    #[test]
    fn test_create_company() {
        let conn = establish_test_connection();
        let service = FinancialDataService::new(&conn);

        let new_company = NewCompany {
            cik: "0000320193".to_string(),
            ticker: Some("AAPL".to_string()),
            name: "Apple Inc.".to_string(),
            legal_name: Some("Apple Inc.".to_string()),
            sic_code: Some("3571".to_string()),
            sic_description: Some("Electronic Computers".to_string()),
            industry: Some("Technology Hardware & Equipment".to_string()),
            sector: Some("Technology".to_string()),
            business_address: None,
            mailing_address: None,
            phone: None,
            website: Some("https://www.apple.com".to_string()),
            state_of_incorporation: Some("CA".to_string()),
            state_of_incorporation_description: Some("California".to_string()),
            fiscal_year_end: Some("0930".to_string()),
            entity_type: Some("Corporation".to_string()),
            entity_size: Some("Large Accelerated Filer".to_string()),
            is_active: true,
        };

        let result = service.create_company(new_company);
        assert!(result.is_ok());

        let company = result.unwrap();
        assert_eq!(company.cik, "0000320193");
        assert_eq!(company.ticker, Some("AAPL".to_string()));
        assert_eq!(company.name, "Apple Inc.");
    }

    #[test]
    fn test_get_company_by_cik() {
        let conn = establish_test_connection();
        let service = FinancialDataService::new(&conn);

        // First create a company
        let new_company = NewCompany {
            cik: "0000320193".to_string(),
            ticker: Some("AAPL".to_string()),
            name: "Apple Inc.".to_string(),
            legal_name: Some("Apple Inc.".to_string()),
            sic_code: Some("3571".to_string()),
            sic_description: Some("Electronic Computers".to_string()),
            industry: Some("Technology Hardware & Equipment".to_string()),
            sector: Some("Technology".to_string()),
            business_address: None,
            mailing_address: None,
            phone: None,
            website: Some("https://www.apple.com".to_string()),
            state_of_incorporation: Some("CA".to_string()),
            state_of_incorporation_description: Some("California".to_string()),
            fiscal_year_end: Some("0930".to_string()),
            entity_type: Some("Corporation".to_string()),
            entity_size: Some("Large Accelerated Filer".to_string()),
            is_active: true,
        };

        service.create_company(new_company).unwrap();

        // Now test retrieval
        let result = service.get_company_by_cik("0000320193");
        assert!(result.is_ok());

        let company = result.unwrap();
        assert!(company.is_some());
        assert_eq!(company.unwrap().cik, "0000320193");
    }
}
