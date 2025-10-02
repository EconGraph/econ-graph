/**
 * REQUIREMENT: Fine-grained permission system for subscription-based features
 * PURPOSE: Define granular permissions that can be individually granted/revoked
 * This enables flexible subscription models without rigid tiers
 */
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Fine-grained permissions for EconGraph features
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EconGraphPermission {
    // === CORE DATA ACCESS ===
    /// Read economic data and indicators
    DataRead,
    /// Write/update economic data (admin only)
    DataWrite,
    /// Access real-time data feeds
    DataRealtime,
    /// Access historical data beyond basic range
    DataHistorical,

    // === NEWS DATA ===
    /// Read basic news headlines and summaries
    NewsRead,
    /// Access premium news sources and detailed articles
    NewsPremium,
    /// Access real-time news feeds and breaking news
    NewsRealTime,
    /// Access historical news archives
    NewsHistorical,

    // === STOCK DATA - TIERED ACCESS ===
    /// Basic stock data (delayed by 15 minutes)
    StockDataBasic,
    /// Real-time stock prices and quotes
    StockDataRealtime,
    /// Intraday stock data with configurable history (days back)
    StockDataIntraday { days_back: u32 },
    /// Options and derivatives data
    StockDataOptions,
    /// Foreign exchange (forex) data
    StockDataForex,
    /// Cryptocurrency data
    StockDataCrypto,
    /// Futures and commodities data
    StockDataFutures,

    // === GRAPH CREATION & MANAGEMENT ===
    /// Create basic graphs and charts
    GraphCreate,
    /// Create advanced visualizations (3D, interactive, etc.)
    GraphAdvanced,
    /// Save graphs to personal library
    GraphSave,
    /// Share graphs with other users
    GraphShare,
    /// Collaborate on shared graphs
    GraphCollaborate,
    /// Create public graphs visible to all users
    GraphPublic,

    // === EXPORT & DOWNLOAD ===
    /// Export graphs as images (PNG, JPG)
    ExportImage,
    /// Export graphs as vector formats (SVG, PDF)
    ExportVector,
    /// Export graphs without watermarks
    ExportWatermarkFree,
    /// Export raw data (CSV, JSON)
    ExportData,
    /// Bulk export multiple graphs
    ExportBulk,

    // === COLLABORATION & NOTES ===
    /// Add notes and annotations to graphs
    NotesAdd,
    /// Edit notes on shared graphs
    NotesEdit,
    /// View notes from other users
    NotesView,
    /// Moderate notes (admin only)
    NotesModerate,
    /// Create shared workspaces
    WorkspaceCreate,
    /// Invite users to workspaces
    WorkspaceInvite,

    // === API & INTEGRATION ===
    /// Access REST API
    ApiAccess,
    /// Access GraphQL API
    ApiGraphQL,
    /// Use webhooks for data updates
    ApiWebhooks,
    /// Access rate limits (requests per hour)
    ApiRateLimit { requests_per_hour: u32 },
    /// Access bulk API endpoints
    ApiBulk,

    // === ANALYTICS & INSIGHTS ===
    /// Access basic analytics
    AnalyticsBasic,
    /// Access advanced analytics and ML insights
    AnalyticsAdvanced,
    /// Access predictive modeling
    AnalyticsPredictive,
    /// Export analytics reports
    AnalyticsExport,

    // === CUSTOMIZATION ===
    /// Use custom themes and branding
    CustomizeThemes,
    /// Upload custom data sources
    CustomizeDataSources,
    /// Create custom indicators
    CustomizeIndicators,
    /// White-label the application
    CustomizeWhiteLabel,

    // === ADMINISTRATION ===
    /// Manage user accounts
    AdminUsers,
    /// Manage system configuration
    AdminSystem,
    /// Access audit logs
    AdminAudit,
    /// Manage security settings
    AdminSecurity,
    /// Manage billing and subscriptions
    AdminBilling,
}

impl fmt::Display for EconGraphPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EconGraphPermission::DataRead => write!(f, "data:read"),
            EconGraphPermission::DataWrite => write!(f, "data:write"),
            EconGraphPermission::DataRealtime => write!(f, "data:realtime"),
            EconGraphPermission::DataHistorical => write!(f, "data:historical"),
            EconGraphPermission::NewsRead => write!(f, "news:read"),
            EconGraphPermission::NewsPremium => write!(f, "news:premium"),
            EconGraphPermission::NewsRealTime => write!(f, "news:realtime"),
            EconGraphPermission::NewsHistorical => write!(f, "news:historical"),
            EconGraphPermission::StockDataBasic => write!(f, "stock:basic"),
            EconGraphPermission::StockDataRealtime => write!(f, "stock:realtime"),
            EconGraphPermission::StockDataIntraday { days_back } => {
                write!(f, "stock:intraday:{}", days_back)
            }
            EconGraphPermission::StockDataOptions => write!(f, "stock:options"),
            EconGraphPermission::StockDataForex => write!(f, "stock:forex"),
            EconGraphPermission::StockDataCrypto => write!(f, "stock:crypto"),
            EconGraphPermission::StockDataFutures => write!(f, "stock:futures"),
            EconGraphPermission::GraphCreate => write!(f, "graph:create"),
            EconGraphPermission::GraphAdvanced => write!(f, "graph:advanced"),
            EconGraphPermission::GraphSave => write!(f, "graph:save"),
            EconGraphPermission::GraphShare => write!(f, "graph:share"),
            EconGraphPermission::GraphCollaborate => write!(f, "graph:collaborate"),
            EconGraphPermission::GraphPublic => write!(f, "graph:public"),
            EconGraphPermission::ExportImage => write!(f, "export:image"),
            EconGraphPermission::ExportVector => write!(f, "export:vector"),
            EconGraphPermission::ExportWatermarkFree => write!(f, "export:watermark-free"),
            EconGraphPermission::ExportData => write!(f, "export:data"),
            EconGraphPermission::ExportBulk => write!(f, "export:bulk"),
            EconGraphPermission::NotesAdd => write!(f, "notes:add"),
            EconGraphPermission::NotesEdit => write!(f, "notes:edit"),
            EconGraphPermission::NotesView => write!(f, "notes:view"),
            EconGraphPermission::NotesModerate => write!(f, "notes:moderate"),
            EconGraphPermission::WorkspaceCreate => write!(f, "workspace:create"),
            EconGraphPermission::WorkspaceInvite => write!(f, "workspace:invite"),
            EconGraphPermission::ApiAccess => write!(f, "api:access"),
            EconGraphPermission::ApiGraphQL => write!(f, "api:graphql"),
            EconGraphPermission::ApiWebhooks => write!(f, "api:webhooks"),
            EconGraphPermission::ApiRateLimit { requests_per_hour } => {
                write!(f, "api:rate-limit:{}", requests_per_hour)
            }
            EconGraphPermission::ApiBulk => write!(f, "api:bulk"),
            EconGraphPermission::AnalyticsBasic => write!(f, "analytics:basic"),
            EconGraphPermission::AnalyticsAdvanced => write!(f, "analytics:advanced"),
            EconGraphPermission::AnalyticsPredictive => write!(f, "analytics:predictive"),
            EconGraphPermission::AnalyticsExport => write!(f, "analytics:export"),
            EconGraphPermission::CustomizeThemes => write!(f, "customize:themes"),
            EconGraphPermission::CustomizeDataSources => write!(f, "customize:data-sources"),
            EconGraphPermission::CustomizeIndicators => write!(f, "customize:indicators"),
            EconGraphPermission::CustomizeWhiteLabel => write!(f, "customize:white-label"),
            EconGraphPermission::AdminUsers => write!(f, "admin:users"),
            EconGraphPermission::AdminSystem => write!(f, "admin:system"),
            EconGraphPermission::AdminAudit => write!(f, "admin:audit"),
            EconGraphPermission::AdminSecurity => write!(f, "admin:security"),
            EconGraphPermission::AdminBilling => write!(f, "admin:billing"),
        }
    }
}

impl EconGraphPermission {
    /// Parse permission from string
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "data:read" => Some(Self::DataRead),
            "data:write" => Some(Self::DataWrite),
            "data:realtime" => Some(Self::DataRealtime),
            "data:historical" => Some(Self::DataHistorical),
            "news:read" => Some(Self::NewsRead),
            "news:premium" => Some(Self::NewsPremium),
            "news:realtime" => Some(Self::NewsRealTime),
            "news:historical" => Some(Self::NewsHistorical),
            "stock:basic" => Some(Self::StockDataBasic),
            "stock:realtime" => Some(Self::StockDataRealtime),
            "stock:options" => Some(Self::StockDataOptions),
            "stock:forex" => Some(Self::StockDataForex),
            "stock:crypto" => Some(Self::StockDataCrypto),
            "stock:futures" => Some(Self::StockDataFutures),
            "graph:create" => Some(Self::GraphCreate),
            "graph:advanced" => Some(Self::GraphAdvanced),
            "graph:save" => Some(Self::GraphSave),
            "graph:share" => Some(Self::GraphShare),
            "graph:collaborate" => Some(Self::GraphCollaborate),
            "graph:public" => Some(Self::GraphPublic),
            "export:image" => Some(Self::ExportImage),
            "export:vector" => Some(Self::ExportVector),
            "export:watermark-free" => Some(Self::ExportWatermarkFree),
            "export:data" => Some(Self::ExportData),
            "export:bulk" => Some(Self::ExportBulk),
            "notes:add" => Some(Self::NotesAdd),
            "notes:edit" => Some(Self::NotesEdit),
            "notes:view" => Some(Self::NotesView),
            "notes:moderate" => Some(Self::NotesModerate),
            "workspace:create" => Some(Self::WorkspaceCreate),
            "workspace:invite" => Some(Self::WorkspaceInvite),
            "api:access" => Some(Self::ApiAccess),
            "api:graphql" => Some(Self::ApiGraphQL),
            "api:webhooks" => Some(Self::ApiWebhooks),
            "api:bulk" => Some(Self::ApiBulk),
            "analytics:basic" => Some(Self::AnalyticsBasic),
            "analytics:advanced" => Some(Self::AnalyticsAdvanced),
            "analytics:predictive" => Some(Self::AnalyticsPredictive),
            "analytics:export" => Some(Self::AnalyticsExport),
            "customize:themes" => Some(Self::CustomizeThemes),
            "customize:data-sources" => Some(Self::CustomizeDataSources),
            "customize:indicators" => Some(Self::CustomizeIndicators),
            "customize:white-label" => Some(Self::CustomizeWhiteLabel),
            "admin:users" => Some(Self::AdminUsers),
            "admin:system" => Some(Self::AdminSystem),
            "admin:audit" => Some(Self::AdminAudit),
            "admin:security" => Some(Self::AdminSecurity),
            "admin:billing" => Some(Self::AdminBilling),
            _ => {
                // Handle rate limit permissions
                if s.starts_with("api:rate-limit:") {
                    if let Ok(requests) = s.strip_prefix("api:rate-limit:").unwrap().parse::<u32>()
                    {
                        return Some(Self::ApiRateLimit {
                            requests_per_hour: requests,
                        });
                    }
                }
                // Handle intraday stock data with days_back parameter
                if s.starts_with("stock:intraday:") {
                    if let Ok(days_back) = s.strip_prefix("stock:intraday:").unwrap().parse::<u32>()
                    {
                        return Some(Self::StockDataIntraday { days_back });
                    }
                }
                None
            }
        }
    }

    /// Get permission category for organization
    pub fn category(&self) -> PermissionCategory {
        match self {
            Self::DataRead | Self::DataWrite | Self::DataRealtime | Self::DataHistorical => {
                PermissionCategory::Data
            }
            Self::NewsRead | Self::NewsPremium | Self::NewsRealTime | Self::NewsHistorical => {
                PermissionCategory::Data
            }
            Self::StockDataBasic
            | Self::StockDataRealtime
            | Self::StockDataIntraday { .. }
            | Self::StockDataOptions
            | Self::StockDataForex
            | Self::StockDataCrypto
            | Self::StockDataFutures => PermissionCategory::Data,
            Self::GraphCreate
            | Self::GraphAdvanced
            | Self::GraphSave
            | Self::GraphShare
            | Self::GraphCollaborate
            | Self::GraphPublic => PermissionCategory::Graphs,
            Self::ExportImage
            | Self::ExportVector
            | Self::ExportWatermarkFree
            | Self::ExportData
            | Self::ExportBulk => PermissionCategory::Export,
            Self::NotesAdd
            | Self::NotesEdit
            | Self::NotesView
            | Self::NotesModerate
            | Self::WorkspaceCreate
            | Self::WorkspaceInvite => PermissionCategory::Collaboration,
            Self::ApiAccess
            | Self::ApiGraphQL
            | Self::ApiWebhooks
            | Self::ApiRateLimit { .. }
            | Self::ApiBulk => PermissionCategory::Api,
            Self::AnalyticsBasic
            | Self::AnalyticsAdvanced
            | Self::AnalyticsPredictive
            | Self::AnalyticsExport => PermissionCategory::Analytics,
            Self::CustomizeThemes
            | Self::CustomizeDataSources
            | Self::CustomizeIndicators
            | Self::CustomizeWhiteLabel => PermissionCategory::Customization,
            Self::AdminUsers
            | Self::AdminSystem
            | Self::AdminAudit
            | Self::AdminSecurity
            | Self::AdminBilling => PermissionCategory::Administration,
        }
    }

    /// Check if permission requires watermark
    pub fn requires_watermark(&self) -> bool {
        matches!(
            self,
            Self::ExportImage | Self::ExportVector | Self::ExportData | Self::ExportBulk
        )
    }

    /// Check if permission is admin-only
    pub fn is_admin_only(&self) -> bool {
        matches!(
            self,
            Self::DataWrite
                | Self::NotesModerate
                | Self::AdminUsers
                | Self::AdminSystem
                | Self::AdminAudit
                | Self::AdminSecurity
                | Self::AdminBilling
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PermissionCategory {
    Data,
    Graphs,
    Export,
    Collaboration,
    Api,
    Analytics,
    Customization,
    Administration,
}

/// Subscription tier definitions with included permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionTier {
    pub name: String,
    pub display_name: String,
    pub permissions: Vec<EconGraphPermission>,
    pub limits: SubscriptionLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionLimits {
    pub graphs_per_month: Option<u32>,
    pub exports_per_month: Option<u32>,
    pub api_requests_per_hour: Option<u32>,
    pub storage_gb: Option<u32>,
    pub team_members: Option<u32>,
}

impl SubscriptionTier {
    /// Get predefined subscription tiers
    pub fn predefined_tiers() -> HashMap<String, SubscriptionTier> {
        let mut tiers = HashMap::new();

        // Free tier
        tiers.insert(
            "free".to_string(),
            SubscriptionTier {
                name: "free".to_string(),
                display_name: "Free".to_string(),
                permissions: vec![
                    EconGraphPermission::DataRead,
                    EconGraphPermission::GraphCreate,
                    EconGraphPermission::GraphSave,
                    EconGraphPermission::ExportImage, // With watermark
                    EconGraphPermission::NotesAdd,
                    EconGraphPermission::AnalyticsBasic,
                ],
                limits: SubscriptionLimits {
                    graphs_per_month: Some(5),
                    exports_per_month: Some(10),
                    api_requests_per_hour: Some(100),
                    storage_gb: Some(1),
                    team_members: Some(1),
                },
            },
        );

        // Pro tier
        tiers.insert(
            "pro".to_string(),
            SubscriptionTier {
                name: "pro".to_string(),
                display_name: "Professional".to_string(),
                permissions: vec![
                    // Free tier permissions
                    EconGraphPermission::DataRead,
                    EconGraphPermission::GraphCreate,
                    EconGraphPermission::GraphSave,
                    EconGraphPermission::ExportImage,
                    EconGraphPermission::NotesAdd,
                    EconGraphPermission::AnalyticsBasic,
                    // Pro tier additions
                    EconGraphPermission::DataHistorical,
                    EconGraphPermission::GraphAdvanced,
                    EconGraphPermission::GraphShare,
                    EconGraphPermission::ExportVector,
                    EconGraphPermission::ExportWatermarkFree, // No watermark!
                    EconGraphPermission::ExportData,
                    EconGraphPermission::NotesEdit,
                    EconGraphPermission::NotesView,
                    EconGraphPermission::WorkspaceCreate,
                    EconGraphPermission::ApiAccess,
                    EconGraphPermission::AnalyticsAdvanced,
                    EconGraphPermission::CustomizeThemes,
                ],
                limits: SubscriptionLimits {
                    graphs_per_month: Some(100),
                    exports_per_month: Some(500),
                    api_requests_per_hour: Some(1000),
                    storage_gb: Some(10),
                    team_members: Some(5),
                },
            },
        );

        // Enterprise tier
        tiers.insert(
            "enterprise".to_string(),
            SubscriptionTier {
                name: "enterprise".to_string(),
                display_name: "Enterprise".to_string(),
                permissions: vec![
                    // All Pro permissions
                    EconGraphPermission::DataRead,
                    EconGraphPermission::DataHistorical,
                    EconGraphPermission::GraphCreate,
                    EconGraphPermission::GraphAdvanced,
                    EconGraphPermission::GraphSave,
                    EconGraphPermission::GraphShare,
                    EconGraphPermission::ExportImage,
                    EconGraphPermission::ExportVector,
                    EconGraphPermission::ExportWatermarkFree,
                    EconGraphPermission::ExportData,
                    EconGraphPermission::NotesAdd,
                    EconGraphPermission::NotesEdit,
                    EconGraphPermission::NotesView,
                    EconGraphPermission::WorkspaceCreate,
                    EconGraphPermission::ApiAccess,
                    EconGraphPermission::AnalyticsAdvanced,
                    EconGraphPermission::CustomizeThemes,
                    // Enterprise additions
                    EconGraphPermission::DataRealtime,
                    EconGraphPermission::GraphCollaborate,
                    EconGraphPermission::GraphPublic,
                    EconGraphPermission::ExportBulk,
                    EconGraphPermission::WorkspaceInvite,
                    EconGraphPermission::ApiGraphQL,
                    EconGraphPermission::ApiWebhooks,
                    EconGraphPermission::ApiBulk,
                    EconGraphPermission::AnalyticsPredictive,
                    EconGraphPermission::AnalyticsExport,
                    EconGraphPermission::CustomizeDataSources,
                    EconGraphPermission::CustomizeIndicators,
                    EconGraphPermission::CustomizeWhiteLabel,
                ],
                limits: SubscriptionLimits {
                    graphs_per_month: None,  // Unlimited
                    exports_per_month: None, // Unlimited
                    api_requests_per_hour: Some(10000),
                    storage_gb: Some(100),
                    team_members: None, // Unlimited
                },
            },
        );

        tiers
    }

    /// Check if tier includes a specific permission
    pub fn has_permission(&self, permission: &EconGraphPermission) -> bool {
        self.permissions.contains(permission)
    }

    /// Get all permissions for this tier
    pub fn get_permissions(&self) -> &[EconGraphPermission] {
        &self.permissions
    }
}

/// Service for managing subscription permissions
pub struct SubscriptionPermissionService {
    tiers: HashMap<String, SubscriptionTier>,
}

impl SubscriptionPermissionService {
    pub fn new() -> Self {
        Self {
            tiers: SubscriptionTier::predefined_tiers(),
        }
    }

    /// Get permissions for a subscription tier
    pub fn get_tier_permissions(&self, tier_name: &str) -> Option<&[EconGraphPermission]> {
        self.tiers.get(tier_name).map(|tier| tier.get_permissions())
    }

    /// Check if a tier has a specific permission
    pub fn tier_has_permission(&self, tier_name: &str, permission: &EconGraphPermission) -> bool {
        self.tiers
            .get(tier_name)
            .map(|tier| tier.has_permission(permission))
            .unwrap_or(false)
    }

    /// Get tier limits
    pub fn get_tier_limits(&self, tier_name: &str) -> Option<&SubscriptionLimits> {
        self.tiers.get(tier_name).map(|tier| &tier.limits)
    }

    /// Create custom tier with specific permissions
    pub fn create_custom_tier(
        &mut self,
        name: String,
        display_name: String,
        permissions: Vec<EconGraphPermission>,
        limits: SubscriptionLimits,
    ) {
        let tier = SubscriptionTier {
            name: name.clone(),
            display_name,
            permissions,
            limits,
        };
        self.tiers.insert(name, tier);
    }

    /// Add permission to existing tier
    pub fn add_permission_to_tier(
        &mut self,
        tier_name: &str,
        permission: EconGraphPermission,
    ) -> bool {
        if let Some(tier) = self.tiers.get_mut(tier_name) {
            if !tier.permissions.contains(&permission) {
                tier.permissions.push(permission);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Remove permission from tier
    pub fn remove_permission_from_tier(
        &mut self,
        tier_name: &str,
        permission: &EconGraphPermission,
    ) -> bool {
        if let Some(tier) = self.tiers.get_mut(tier_name) {
            tier.permissions.retain(|p| p != permission);
            true
        } else {
            false
        }
    }

    /// Get all available tiers
    pub fn get_all_tiers(&self) -> &HashMap<String, SubscriptionTier> {
        &self.tiers
    }
}

impl Default for SubscriptionPermissionService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_parsing() {
        assert_eq!(
            EconGraphPermission::from_string("export:watermark-free"),
            Some(EconGraphPermission::ExportWatermarkFree)
        );
        assert_eq!(
            EconGraphPermission::from_string("api:rate-limit:1000"),
            Some(EconGraphPermission::ApiRateLimit {
                requests_per_hour: 1000
            })
        );
    }

    #[test]
    fn test_subscription_tiers() {
        let service = SubscriptionPermissionService::new();

        // Free tier should not have watermark-free export
        assert!(!service.tier_has_permission("free", &EconGraphPermission::ExportWatermarkFree));

        // Pro tier should have watermark-free export
        assert!(service.tier_has_permission("pro", &EconGraphPermission::ExportWatermarkFree));

        // Enterprise should have all permissions
        assert!(
            service.tier_has_permission("enterprise", &EconGraphPermission::ExportWatermarkFree)
        );
        assert!(service.tier_has_permission("enterprise", &EconGraphPermission::GraphCollaborate));
    }
}
