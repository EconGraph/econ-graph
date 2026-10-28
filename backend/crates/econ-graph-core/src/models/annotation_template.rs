use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::AnnotationType;
use crate::schema::annotation_templates;

/// Stored template for reusable annotation content.
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = annotation_templates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AnnotationTemplate {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub annotation_type: AnnotationType,
    pub template_content: String,
    pub usage_count: Option<i32>,
    pub is_active: bool,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Values needed to store an annotation template.
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = annotation_templates)]
pub struct NewAnnotationTemplate {
    pub name: String,
    pub description: Option<String>,
    pub annotation_type: AnnotationType,
    pub template_content: String,
    pub is_active: bool,
    pub created_by: Uuid,
}

impl NewAnnotationTemplate {
    /// Create an active template owned by its creator.
    pub fn new(
        name: String,
        template_content: String,
        annotation_type: AnnotationType,
        created_by: Uuid,
    ) -> Self {
        Self {
            name,
            description: None,
            template_content,
            annotation_type,
            is_active: true,
            created_by,
        }
    }
    /// Describe when the template should be used.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

/// Pre-built annotation templates for common analysis types
pub struct AnnotationTemplateLibrary;

impl AnnotationTemplateLibrary {
    /// Get default templates for financial analysis
    pub fn get_default_templates() -> Vec<DefaultTemplate> {
        vec![
            DefaultTemplate {
                name: "Revenue Growth Analysis".to_string(),
                description: Some("Template for analyzing revenue growth trends and drivers".to_string()),
                template_content: "Revenue Analysis:\n- Growth Rate: {growth_rate}%\n- Key Drivers: {drivers}\n- Industry Comparison: {industry_comparison}\n- Outlook: {outlook}".to_string(),
                annotation_type: AnnotationType::RevenueGrowth,
                tags: vec!["revenue".to_string(), "growth".to_string(), "analysis".to_string()],
            },
            DefaultTemplate {
                name: "Cost Structure Review".to_string(),
                description: Some("Template for reviewing cost structure and efficiency".to_string()),
                template_content: "Cost Analysis:\n- Cost Trends: {cost_trends}\n- Efficiency Metrics: {efficiency}\n- Cost Concerns: {concerns}\n- Recommendations: {recommendations}".to_string(),
                annotation_type: AnnotationType::CostConcern,
                tags: vec!["costs".to_string(), "efficiency".to_string(), "review".to_string()],
            },
            DefaultTemplate {
                name: "Cash Flow Assessment".to_string(),
                description: Some("Template for assessing cash flow quality and sustainability".to_string()),
                template_content: "Cash Flow Analysis:\n- Operating Cash Flow: {operating_cf}\n- Free Cash Flow: {free_cf}\n- Quality Assessment: {quality}\n- Sustainability: {sustainability}".to_string(),
                annotation_type: AnnotationType::CashFlow,
                tags: vec!["cash_flow".to_string(), "liquidity".to_string(), "assessment".to_string()],
            },
            DefaultTemplate {
                name: "Balance Sheet Strength".to_string(),
                description: Some("Template for evaluating balance sheet strength and leverage".to_string()),
                template_content: "Balance Sheet Analysis:\n- Leverage Ratio: {leverage}\n- Liquidity Position: {liquidity}\n- Asset Quality: {asset_quality}\n- Risk Assessment: {risk}".to_string(),
                annotation_type: AnnotationType::BalanceSheet,
                tags: vec!["balance_sheet".to_string(), "leverage".to_string(), "risk".to_string()],
            },
            DefaultTemplate {
                name: "One-Time Item Identification".to_string(),
                description: Some("Template for identifying and analyzing one-time items".to_string()),
                template_content: "One-Time Item:\n- Description: {description}\n- Impact: {impact}\n- Normalization: {normalization}\n- Recurrence Risk: {recurrence}".to_string(),
                annotation_type: AnnotationType::OneTimeItem,
                tags: vec!["one_time".to_string(), "normalization".to_string(), "adjustment".to_string()],
            },
            DefaultTemplate {
                name: "Industry Context".to_string(),
                description: Some("Template for providing industry-specific context and benchmarks".to_string()),
                template_content: "Industry Context:\n- Industry Trends: {trends}\n- Peer Comparison: {peers}\n- Market Position: {position}\n- Competitive Advantage: {advantage}".to_string(),
                annotation_type: AnnotationType::IndustryContext,
                tags: vec!["industry".to_string(), "context".to_string(), "benchmark".to_string()],
            },
            DefaultTemplate {
                name: "Risk Assessment".to_string(),
                description: Some("Template for identifying and assessing financial risks".to_string()),
                template_content: "Risk Assessment:\n- Risk Type: {risk_type}\n- Probability: {probability}\n- Impact: {impact}\n- Mitigation: {mitigation}".to_string(),
                annotation_type: AnnotationType::Risk,
                tags: vec!["risk".to_string(), "assessment".to_string(), "mitigation".to_string()],
            },
            DefaultTemplate {
                name: "Investment Opportunity".to_string(),
                description: Some("Template for identifying investment opportunities".to_string()),
                template_content: "Investment Opportunity:\n- Opportunity Type: {opportunity_type}\n- Potential Value: {value}\n- Timeline: {timeline}\n- Key Catalysts: {catalysts}".to_string(),
                annotation_type: AnnotationType::Opportunity,
                tags: vec!["opportunity".to_string(), "investment".to_string(), "catalyst".to_string()],
            },
        ]
    }
}

/// Default template structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultTemplate {
    pub name: String,
    pub description: Option<String>,
    pub template_content: String,
    pub annotation_type: AnnotationType,
    pub tags: Vec<String>,
}

/// Filter for querying annotation templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationTemplateFilter {
    pub created_by: Option<Uuid>,
    pub annotation_type: Option<AnnotationType>,
    pub is_active: Option<bool>,
    pub name_contains: Option<String>,
}

impl Default for AnnotationTemplateFilter {
    fn default() -> Self {
        Self {
            created_by: None,
            annotation_type: None,
            is_active: Some(true),
            name_contains: None,
        }
    }
}
