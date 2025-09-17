//! # Import Mapping Module
//!
//! This module provides a centralized mapping of all imports used across the GraphQL crate.
//! This ensures consistency and makes it easy to update import paths when the crate structure changes.
//!
//! # Design Principles
//!
//! 1. **Centralized Imports**: All external crate imports are defined here
//! 2. **Re-export Pattern**: Common types are re-exported for easy access
//! 3. **Type Safety**: All imports are properly typed and documented
//! 4. **Maintainability**: Changes to crate structure only require updates here
//!
//! # Usage
//!
//! ```rust,no_run
//! use crate::imports::*;
//!
//! // Now you can use all the imported types directly
//! let pool: DatabasePool = get_pool();
//! let user: User = get_user();
//! ```

// Core crate imports
pub use econ_graph_core::{
    auth_models::{AuthProvider, User as AuthUser, UserRole},
    database::DatabasePool,
    error::{AppError, AppResult},
    // Additional imports for missing modules
    models as core_models,
    models::{
        AnnotationComment,
        // Chart annotations
        ChartAnnotation,
        ChartCollaborator,
        CorrelationConnection,
        CorrelationNetworkNode,
        // Global analysis
        Country,
        CountryCorrelation,
        CountryImpactDetail,
        CountryWithEconomicData,
        DataPoint,
        DataQueryParams,
        DataSource,
        // Data transformations
        DataTransformation,
        // Core data models
        EconomicSeries,
        EventCountryImpact,
        GlobalEconomicEvent,
        GlobalEventWithImpacts,
        // User management
        NewUser,
        // Search ordering
        SearchSortOrder,
        SearchSuggestion,
        // Search parameters
        SeriesSearchParams,
        // Search and discovery
        SeriesSearchResult,
        SuggestionType,
        TradePartner,
        User,
    },
    search,
};

// Re-export the models module for easy access
pub use econ_graph_core::models;

// Services crate imports
pub use econ_graph_services::services::{
    collaboration_service::{CollaborationService, PermissionLevel},
    crawler::{crawler_service, simple_crawler_service},
    global_analysis_service::GlobalAnalysisService,
    queue_service,
    // Core services
    search_service::SearchService,
    series_service,
};

// GraphQL framework imports
pub use async_graphql::{
    Context, EmptyMutation, EmptySubscription, Enum, Error as GraphQLError, InputObject, Object,
    Result, Schema, SimpleObject, ID,
};

// Standard library and external crate imports
pub use bigdecimal::BigDecimal;
pub use chrono::{DateTime, NaiveDate, Utc};
pub use diesel::SelectableHelper;
pub use rust_decimal::Decimal;
pub use serde::{Deserialize, Serialize};
pub use std::sync::Arc;
pub use uuid::Uuid;

// Re-export commonly used types for convenience
// Note: These are already imported above, so we don't need to redefine them

// Re-export GraphQL context utilities
pub use crate::graphql::context::{current_user, require_admin, GraphQLContext};
