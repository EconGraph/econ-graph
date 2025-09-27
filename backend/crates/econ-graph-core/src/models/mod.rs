//! # Data Models
//!
//! This module contains all data models and database entities for the `EconGraph` system.
//! It provides type-safe representations of database tables with comprehensive business logic.
//!
//! ## Core Models
//!
//! - **Economic Data**: Economic series, data points, and financial statements
//! - **User Management**: Users, authentication, and collaboration features
//! - **Company Data**: Company information and financial metrics
//! - **Search & Analysis**: Search functionality and global analysis tools
//! - **Educational Content**: Learning modules and educational resources
//!
//! ## Features
//!
//! - **Type Safety**: Compile-time validation of data structures
//! - **Database Integration**: Seamless Diesel ORM integration
//! - **Validation**: Input validation with validator crate
//! - **Serialization**: Full serde support for JSON/API responses
//! - **Business Logic**: Rich domain models with business rules
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_core::models::{Company, DataPoint, EconomicSeries};
//! use econ_graph_core::models::NewCompany;
//!
//! // Create a new company
//! let new_company = NewCompany {
//!     name: "Apple Inc".to_string(),
//!     ticker: "AAPL".to_string(),
//!     // ... other fields
//! };
//! ```

pub mod admin;
pub mod annotation_assignment;
pub mod annotation_reply;
pub mod annotation_template;
pub mod company;
pub mod crawl_attempt;
pub mod crawl_queue;
pub mod data_point;
pub mod data_source;
pub mod economic_series;
pub mod educational_content;
pub mod financial_annotation;
pub mod financial_line_item;
pub mod financial_ratios;
pub mod financial_statement;
pub mod global_analysis;
pub mod search;
pub mod series_metadata;
pub mod user;
pub mod xbrl_taxonomy_schema;

pub use annotation_assignment::*;
pub use annotation_reply::*;
pub use annotation_template::*;
pub use company::*;
pub use crawl_attempt::*;
pub use crawl_queue::*;
pub use data_point::*;
pub use data_source::*;
pub use economic_series::*;
pub use educational_content::{
    AssessmentQuestion, ContentSection, EducationalModule, EducationalResource, ExpertInsight,
    InteractiveExercise, LearningAchievement, LearningCategory, LearningDifficulty, LearningPath,
    LearningProgress, ResourceType,
};
pub use financial_annotation::*;
pub use financial_line_item::*;
pub use financial_ratios::*;
pub use financial_statement::*;
pub use global_analysis::*;
pub use search::*;
pub use series_metadata::*;
pub use user::{AnnotationComment, ChartAnnotation, ChartCollaborator, NewUser, User, UserSession};
pub use xbrl_taxonomy_schema::*;
