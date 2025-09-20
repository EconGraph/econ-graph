use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Educational resource (link, document, video, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EducationalResource {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub resource_type: ResourceType,
    pub url: Option<String>,
    pub content: Option<String>,
    pub duration_minutes: Option<i32>,
    pub difficulty: LearningDifficulty,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Type of educational resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Article,
    Video,
    Interactive,
    Document,
    ExternalLink,
    PracticeExercise,
}

/// Educational content for financial analysis learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EducationalModule {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub difficulty: LearningDifficulty,
    pub category: LearningCategory,
    pub estimated_duration_minutes: i32,
    pub prerequisites: Vec<Uuid>, // IDs of prerequisite modules
    pub learning_objectives: Vec<String>,
    pub content_sections: Vec<ContentSection>,
    pub interactive_exercises: Vec<InteractiveExercise>,
    pub assessment_questions: Vec<AssessmentQuestion>,
    pub resources: Vec<EducationalResource>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub author: String,
    pub rating: f64,
    pub completion_count: i32,
}

/// Content section within a learning module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSection {
    pub id: Uuid,
    pub title: String,
    pub content_type: ContentType,
    pub content: String,
    pub order_index: i32,
    pub interactive_elements: Vec<InteractiveElement>,
    pub related_ratios: Vec<String>,
}

/// Interactive element within content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    pub id: Uuid,
    pub element_type: InteractiveElementType,
    pub title: String,
    pub content: String,
    pub configuration: serde_json::Value,
    pub correct_answer: Option<serde_json::Value>,
    pub explanation: Option<String>,
}

/// Interactive exercise for hands-on learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveExercise {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub exercise_type: ExerciseType,
    pub instructions: String,
    pub data: serde_json::Value, // Financial data for the exercise
    pub expected_calculations: Vec<ExpectedCalculation>,
    pub hints: Vec<String>,
    pub solution: serde_json::Value,
    pub difficulty: LearningDifficulty,
    pub estimated_time_minutes: i32,
}

/// Expected calculation in an exercise
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedCalculation {
    pub ratio_name: String,
    pub formula: String,
    pub expected_value: f64,
    pub tolerance: f64, // Acceptable range for the answer
    pub explanation: String,
}

/// Assessment question for testing knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentQuestion {
    pub id: Uuid,
    pub question_type: QuestionType,
    pub question: String,
    pub options: Vec<String>, // For multiple choice
    pub correct_answer: serde_json::Value,
    pub explanation: String,
    pub difficulty: LearningDifficulty,
    pub related_ratios: Vec<String>,
    pub points: i32,
}

/// Learning path for structured education
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPath {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub target_audience: TargetAudience,
    pub estimated_duration_hours: i32,
    pub modules: Vec<LearningPathModule>,
    pub prerequisites: Vec<String>,
    pub learning_outcomes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub author: String,
    pub rating: f64,
    pub completion_count: i32,
}

/// Module within a learning path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPathModule {
    pub module_id: Uuid,
    pub order_index: i32,
    pub is_required: bool,
    pub estimated_duration_minutes: i32,
}

/// User's learning progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningProgress {
    pub id: Uuid,
    pub user_id: Uuid,
    pub module_id: Option<Uuid>,
    pub learning_path_id: Option<Uuid>,
    pub status: LearningStatus,
    pub progress_percentage: f64,
    pub time_spent_minutes: i32,
    pub completed_sections: Vec<Uuid>,
    pub completed_exercises: Vec<Uuid>,
    pub quiz_scores: Vec<QuizScore>,
    pub started_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Quiz score for assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizScore {
    pub question_id: Uuid,
    pub user_answer: serde_json::Value,
    pub is_correct: bool,
    pub points_earned: i32,
    pub time_taken_seconds: i32,
    pub attempted_at: DateTime<Utc>,
}

/// User's learning achievements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningAchievement {
    pub id: Uuid,
    pub user_id: Uuid,
    pub achievement_type: AchievementType,
    pub title: String,
    pub description: String,
    pub criteria: String,
    pub earned_at: DateTime<Utc>,
    pub module_id: Option<Uuid>,
    pub learning_path_id: Option<Uuid>,
    pub badge_url: Option<String>,
}

/// Expert insights and case studies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertInsight {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub insight_type: InsightType,
    pub expert_name: String,
    pub expert_credentials: String,
    pub company_examples: Vec<String>,
    pub related_ratios: Vec<String>,
    pub key_takeaways: Vec<String>,
    pub difficulty: LearningDifficulty,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub rating: f64,
    pub view_count: i32,
}

/// Enums for educational content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LearningDifficulty {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LearningCategory {
    FinancialStatements,
    RatioAnalysis,
    Valuation,
    CashFlowAnalysis,
    IndustryAnalysis,
    InvestmentPhilosophy,
    WarrenBuffett,
    ModernAnalytics,
    EnterpriseValue,
    FreeCashFlow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContentType {
    Text,
    Video,
    Interactive,
    CaseStudy,
    Example,
    Exercise,
    Quiz,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InteractiveElementType {
    Calculator,
    Chart,
    Comparison,
    Scenario,
    Quiz,
    DragAndDrop,
    FillInTheBlank,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExerciseType {
    RatioCalculation,
    CompanyComparison,
    IndustryAnalysis,
    ValuationExercise,
    CashFlowAnalysis,
    TrendAnalysis,
    PeerBenchmarking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuestionType {
    MultipleChoice,
    TrueFalse,
    FillInTheBlank,
    Calculation,
    Essay,
    CaseStudy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TargetAudience {
    IndividualInvestor,
    ProfessionalAnalyst,
    InvestmentBanker,
    PortfolioManager,
    Student,
    CorporateFinance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LearningStatus {
    NotStarted,
    InProgress,
    Completed,
    Paused,
    Abandoned,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AchievementType {
    ModuleCompletion,
    PathCompletion,
    PerfectScore,
    SpeedCompletion,
    Streak,
    ExpertLevel,
    CommunityContributor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InsightType {
    CaseStudy,
    BestPractice,
    CommonMistake,
    IndustryInsight,
    ExpertOpinion,
    HistoricalAnalysis,
    TrendAnalysis,
}

/// Educational content library with predefined content
pub struct EducationalContentLibrary;

impl EducationalContentLibrary {
    /// Get the "Warren Buffett's Investment Philosophy" learning path
    pub fn get_warren_buffett_path() -> LearningPath {
        LearningPath {
            id: Uuid::new_v4(),
            title: "Warren Buffett's Investment Philosophy".to_string(),
            description: "Master the investment principles and financial analysis techniques used by the Oracle of Omaha".to_string(),
            target_audience: TargetAudience::IndividualInvestor,
            estimated_duration_hours: 12,
            modules: vec![
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 1,
                    is_required: true,
                    estimated_duration_minutes: 60,
                },
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 2,
                    is_required: true,
                    estimated_duration_minutes: 90,
                },
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 3,
                    is_required: true,
                    estimated_duration_minutes: 120,
                },
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 4,
                    is_required: true,
                    estimated_duration_minutes: 90,
                },
            ],
            prerequisites: vec![
                "Basic understanding of financial statements".to_string(),
                "Familiarity with basic accounting concepts".to_string(),
            ],
            learning_outcomes: vec![
                "Understand Buffett's value investing principles".to_string(),
                "Master free cash flow analysis".to_string(),
                "Learn to identify moats and competitive advantages".to_string(),
                "Apply Buffett's criteria to investment decisions".to_string(),
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            author: "EconGraph Education Team".to_string(),
            rating: 4.9,
            completion_count: 1250,
        }
    }

    /// Get the "Modern Valuation Techniques" learning path
    pub fn get_modern_valuation_path() -> LearningPath {
        LearningPath {
            id: Uuid::new_v4(),
            title: "Modern Valuation Techniques for Forward-Thinking Analysts".to_string(),
            description: "Learn the advanced valuation methods preferred by professional analysts, including Enterprise Value metrics and DCF modeling".to_string(),
            target_audience: TargetAudience::ProfessionalAnalyst,
            estimated_duration_hours: 16,
            modules: vec![
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 1,
                    is_required: true,
                    estimated_duration_minutes: 90,
                },
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 2,
                    is_required: true,
                    estimated_duration_minutes: 120,
                },
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 3,
                    is_required: true,
                    estimated_duration_minutes: 150,
                },
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 4,
                    is_required: true,
                    estimated_duration_minutes: 120,
                },
                LearningPathModule {
                    module_id: Uuid::new_v4(),
                    order_index: 5,
                    is_required: true,
                    estimated_duration_minutes: 180,
                },
            ],
            prerequisites: vec![
                "Advanced understanding of financial statements".to_string(),
                "Knowledge of basic valuation concepts".to_string(),
                "Familiarity with Excel or similar tools".to_string(),
            ],
            learning_outcomes: vec![
                "Master Enterprise Value calculations and applications".to_string(),
                "Understand why EV/EBITDA is preferred over P/E ratios".to_string(),
                "Build comprehensive DCF models".to_string(),
                "Apply scenario analysis and sensitivity testing".to_string(),
                "Compare companies using modern valuation metrics".to_string(),
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            author: "EconGraph Education Team".to_string(),
            rating: 4.8,
            completion_count: 890,
        }
    }

    /// Get expert insights about Enterprise Value
    pub fn get_enterprise_value_insights() -> Vec<ExpertInsight> {
        vec![
            ExpertInsight {
                id: Uuid::new_v4(),
                title: "Why Professional Analysts Prefer EV/EBITDA Over P/E Ratios".to_string(),
                content: "Enterprise Value to EBITDA has become the gold standard for valuation in professional finance because it eliminates the distortions caused by different capital structures, accounting methods, and tax rates. Unlike P/E ratios, EV/EBITDA provides a true apples-to-apples comparison between companies.".to_string(),
                insight_type: InsightType::ExpertOpinion,
                expert_name: "Sarah Chen".to_string(),
                expert_credentials: "Former Goldman Sachs Investment Banking, CFA".to_string(),
                company_examples: vec![
                    "Apple vs Microsoft comparison".to_string(),
                    "Tesla vs Ford valuation analysis".to_string(),
                ],
                related_ratios: vec![
                    "enterprise_value_to_ebitda".to_string(),
                    "enterprise_value_to_sales".to_string(),
                    "enterprise_value_to_free_cash_flow".to_string(),
                ],
                key_takeaways: vec![
                    "EV/EBITDA normalizes for capital structure differences".to_string(),
                    "More reliable for cross-company comparisons".to_string(),
                    "Preferred by M&A professionals and institutional investors".to_string(),
                ],
                difficulty: LearningDifficulty::Advanced,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                rating: 4.9,
                view_count: 2340,
            },
            ExpertInsight {
                id: Uuid::new_v4(),
                title: "Warren Buffett's Free Cash Flow Obsession".to_string(),
                content: "Warren Buffett has repeatedly emphasized that free cash flow is the ultimate measure of business value. Unlike earnings, which can be manipulated through accounting tricks, free cash flow represents the actual cash a company generates that can be returned to shareholders or reinvested for growth.".to_string(),
                insight_type: InsightType::CaseStudy,
                expert_name: "Michael Rodriguez".to_string(),
                expert_credentials: "Berkshire Hathaway Analyst, Value Investor".to_string(),
                company_examples: vec![
                    "Apple's $100B+ annual free cash flow".to_string(),
                    "Coca-Cola's consistent FCF generation".to_string(),
                ],
                related_ratios: vec![
                    "free_cash_flow".to_string(),
                    "free_cash_flow_yield".to_string(),
                    "cash_flow_return_on_investment".to_string(),
                ],
                key_takeaways: vec![
                    "Free cash flow is harder to manipulate than earnings".to_string(),
                    "Shows actual cash available to shareholders".to_string(),
                    "Key metric for dividend sustainability and buybacks".to_string(),
                ],
                difficulty: LearningDifficulty::Intermediate,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                rating: 4.8,
                view_count: 1890,
            },
        ]
    }

    /// Get interactive exercises for ratio calculation
    pub fn get_ratio_calculation_exercises() -> Vec<InteractiveExercise> {
        vec![
            InteractiveExercise {
                id: Uuid::new_v4(),
                title: "Calculate Apple's EV/EBITDA".to_string(),
                description: "Using Apple's financial data, calculate the Enterprise Value to EBITDA ratio and compare it to industry benchmarks".to_string(),
                exercise_type: ExerciseType::RatioCalculation,
                instructions: "Given Apple's market cap, debt, cash, and EBITDA, calculate the EV/EBITDA ratio. Consider why this metric is preferred by analysts over P/E ratios.".to_string(),
                data: serde_json::json!({
                    "market_capitalization": 3000000000000,
                    "total_debt": 120000000000,
                    "cash_and_equivalents": 200000000000,
                    "ebitda": 120000000000,
                    "net_income": 100000000000,
                    "shares_outstanding": 15000000000
                }),
                expected_calculations: vec![
                    ExpectedCalculation {
                        ratio_name: "Enterprise Value".to_string(),
                        formula: "Market Cap + Total Debt - Cash".to_string(),
                        expected_value: 2920000000000.0,
                        tolerance: 1000000000.0,
                        explanation: "Enterprise Value includes debt and subtracts cash to show true acquisition cost".to_string(),
                    },
                    ExpectedCalculation {
                        ratio_name: "EV/EBITDA".to_string(),
                        formula: "Enterprise Value / EBITDA".to_string(),
                        expected_value: 24.33,
                        tolerance: 0.1,
                        explanation: "EV/EBITDA shows how many years of EBITDA it would take to pay for the company".to_string(),
                    },
                ],
                hints: vec![
                    "Remember to add debt and subtract cash from market cap".to_string(),
                    "EV/EBITDA is preferred because it's not affected by capital structure".to_string(),
                    "Compare the result to Apple's historical EV/EBITDA range of 15-25x".to_string(),
                ],
                solution: serde_json::json!({
                    "enterprise_value": 2920000000000,
                    "ev_ebitda": 24.33,
                    "interpretation": "Apple's EV/EBITDA of 24.33x is within its historical range, reflecting strong cash generation and growth prospects"
                }),
                difficulty: LearningDifficulty::Intermediate,
                estimated_time_minutes: 15,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_difficulty_enum() {
        assert_eq!(LearningDifficulty::Beginner, LearningDifficulty::Beginner);
        assert_ne!(LearningDifficulty::Beginner, LearningDifficulty::Advanced);
    }

    #[test]
    fn test_warren_buffett_path_creation() {
        let path = EducationalContentLibrary::get_warren_buffett_path();
        assert_eq!(path.title, "Warren Buffett's Investment Philosophy");
        assert_eq!(path.target_audience, TargetAudience::IndividualInvestor);
        assert_eq!(path.estimated_duration_hours, 12);
        assert!(!path.modules.is_empty());
    }

    #[test]
    fn test_modern_valuation_path_creation() {
        let path = EducationalContentLibrary::get_modern_valuation_path();
        assert_eq!(
            path.title,
            "Modern Valuation Techniques for Forward-Thinking Analysts"
        );
        assert_eq!(path.target_audience, TargetAudience::ProfessionalAnalyst);
        assert_eq!(path.estimated_duration_hours, 16);
        assert!(path
            .learning_outcomes
            .contains(&"Master Enterprise Value calculations and applications".to_string()));
    }

    #[test]
    fn test_expert_insights_creation() {
        let insights = EducationalContentLibrary::get_enterprise_value_insights();
        assert!(!insights.is_empty());
        assert!(insights.iter().any(|i| i.title.contains("EV/EBITDA")));
        assert!(insights.iter().any(|i| i.title.contains("Free Cash Flow")));
    }

    #[test]
    fn test_interactive_exercises_creation() {
        let exercises = EducationalContentLibrary::get_ratio_calculation_exercises();
        assert!(!exercises.is_empty());
        assert!(exercises.iter().any(|e| e.title.contains("EV/EBITDA")));
        assert!(exercises
            .iter()
            .any(|e| e.exercise_type == ExerciseType::RatioCalculation));
    }
}
