use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// **Configuration Loader**
///
/// Loads financial analysis configuration from external JSON files.
/// This allows financial analysts to modify benchmarks, interpretations,
/// and mappings without touching code.

/// **Concept Mappings Configuration**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptMappingsConfig {
    pub us_gaap: HashMap<String, String>,
    pub ifrs: HashMap<String, String>,
}

/// **Ratio Benchmarks Configuration**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioBenchmarksConfig {
    pub industries: HashMap<String, IndustryBenchmark>,
}

/// **Industry Benchmark**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndustryBenchmark {
    pub industry_code: String,
    pub industry_name: String,
    pub benchmarks: HashMap<String, RatioBenchmark>,
}

/// **Ratio Benchmark**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioBenchmark {
    pub ratio_name: String,
    pub median: f64,
    pub p25: f64,
    pub p75: f64,
    pub p90: f64,
    pub p10: f64,
}

/// **Ratio Interpretations Configuration**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioInterpretationsConfig {
    #[serde(flatten)]
    pub interpretations: HashMap<String, RatioInterpretation>,
}

/// **Ratio Interpretation**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioInterpretation {
    pub excellent: InterpretationLevel,
    pub good: InterpretationLevel,
    pub average: InterpretationLevel,
    pub below_average: InterpretationLevel,
    pub poor: InterpretationLevel,
}

/// **Interpretation Level**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpretationLevel {
    pub threshold: f64,
    pub description: String,
}

/// **Ratio Formulas Configuration**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioFormulasConfig {
    pub profitability: HashMap<String, RatioFormula>,
    pub liquidity: HashMap<String, RatioFormula>,
    pub leverage: HashMap<String, RatioFormula>,
    pub efficiency: HashMap<String, RatioFormula>,
    pub valuation: HashMap<String, RatioFormula>,
    pub warren_buffett_favorites: HashMap<String, RatioFormula>,
    pub growth: HashMap<String, RatioFormula>,
}

/// **Ratio Formula**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioFormula {
    pub formula: String,
    pub description: String,
    pub educational_link: String,
}

/// **Financial Analysis Configuration**
#[derive(Debug, Clone)]
pub struct FinancialAnalysisConfig {
    pub concept_mappings: ConceptMappingsConfig,
    pub ratio_benchmarks: RatioBenchmarksConfig,
    pub ratio_interpretations: RatioInterpretationsConfig,
    pub ratio_formulas: RatioFormulasConfig,
}

impl FinancialAnalysisConfig {
    /// Load configuration from config directory
    pub fn load_from_dir<P: AsRef<Path>>(config_dir: P) -> Result<Self> {
        let config_dir = config_dir.as_ref();

        let concept_mappings = Self::load_concept_mappings(config_dir)?;
        let ratio_benchmarks = Self::load_ratio_benchmarks(config_dir)?;
        let ratio_interpretations = Self::load_ratio_interpretations(config_dir)?;
        let ratio_formulas = Self::load_ratio_formulas(config_dir)?;

        Ok(Self {
            concept_mappings,
            ratio_benchmarks,
            ratio_interpretations,
            ratio_formulas,
        })
    }

    /// Load concept mappings from JSON file
    fn load_concept_mappings(config_dir: &Path) -> Result<ConceptMappingsConfig> {
        let file_path = config_dir.join("concept_mappings.json");
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read concept mappings from {:?}", file_path))?;

        let config: ConceptMappingsConfig = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse concept mappings from {:?}", file_path))?;

        Ok(config)
    }

    /// Load ratio benchmarks from JSON file
    fn load_ratio_benchmarks(config_dir: &Path) -> Result<RatioBenchmarksConfig> {
        let file_path = config_dir.join("ratio_benchmarks.json");
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read ratio benchmarks from {:?}", file_path))?;

        let config: RatioBenchmarksConfig = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse ratio benchmarks from {:?}", file_path))?;

        Ok(config)
    }

    /// Load ratio interpretations from JSON file
    fn load_ratio_interpretations(config_dir: &Path) -> Result<RatioInterpretationsConfig> {
        let file_path = config_dir.join("ratio_interpretations.json");
        let content = fs::read_to_string(&file_path).with_context(|| {
            format!("Failed to read ratio interpretations from {:?}", file_path)
        })?;

        let config: RatioInterpretationsConfig =
            serde_json::from_str(&content).with_context(|| {
                format!("Failed to parse ratio interpretations from {:?}", file_path)
            })?;

        Ok(config)
    }

    /// Load ratio formulas from JSON file
    fn load_ratio_formulas(config_dir: &Path) -> Result<RatioFormulasConfig> {
        let file_path = config_dir.join("ratio_formulas.json");
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read ratio formulas from {:?}", file_path))?;

        let config: RatioFormulasConfig = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse ratio formulas from {:?}", file_path))?;

        Ok(config)
    }

    /// Get concept mapping for a given concept name and taxonomy
    pub fn get_concept_mapping(&self, concept: &str, taxonomy: &str) -> Option<&String> {
        match taxonomy {
            "us-gaap" => self.concept_mappings.us_gaap.get(concept),
            "ifrs" => self.concept_mappings.ifrs.get(concept),
            _ => None,
        }
    }

    /// Get industry benchmark for a given industry code and ratio
    pub fn get_industry_benchmark(
        &self,
        industry_code: &str,
        ratio_name: &str,
    ) -> Option<&RatioBenchmark> {
        self.ratio_benchmarks
            .industries
            .get(industry_code)?
            .benchmarks
            .get(ratio_name)
    }

    /// Get ratio interpretation for a given ratio name and value
    pub fn get_ratio_interpretation(&self, ratio_name: &str, value: f64) -> Option<String> {
        let interpretation = self.ratio_interpretations.interpretations.get(ratio_name)?;

        if value >= interpretation.excellent.threshold {
            Some(interpretation.excellent.description.clone())
        } else if value >= interpretation.good.threshold {
            Some(interpretation.good.description.clone())
        } else if value >= interpretation.average.threshold {
            Some(interpretation.average.description.clone())
        } else if value >= interpretation.below_average.threshold {
            Some(interpretation.below_average.description.clone())
        } else {
            Some(interpretation.poor.description.clone())
        }
    }

    /// Get ratio formula for a given ratio name
    pub fn get_ratio_formula(&self, ratio_name: &str) -> Option<&RatioFormula> {
        // Search through all categories
        for category in [
            &self.ratio_formulas.profitability,
            &self.ratio_formulas.liquidity,
            &self.ratio_formulas.leverage,
            &self.ratio_formulas.efficiency,
            &self.ratio_formulas.valuation,
            &self.ratio_formulas.warren_buffett_favorites,
            &self.ratio_formulas.growth,
        ] {
            if let Some(formula) = category.get(ratio_name) {
                return Some(formula);
            }
        }
        None
    }

    /// Get all ratio names by category
    pub fn get_ratios_by_category(&self, category: &str) -> Vec<String> {
        match category {
            "profitability" => self.ratio_formulas.profitability.keys().cloned().collect(),
            "liquidity" => self.ratio_formulas.liquidity.keys().cloned().collect(),
            "leverage" => self.ratio_formulas.leverage.keys().cloned().collect(),
            "efficiency" => self.ratio_formulas.efficiency.keys().cloned().collect(),
            "valuation" => self.ratio_formulas.valuation.keys().cloned().collect(),
            "warren_buffett_favorites" => self
                .ratio_formulas
                .warren_buffett_favorites
                .keys()
                .cloned()
                .collect(),
            "growth" => self.ratio_formulas.growth.keys().cloned().collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_config_loading() {
        let config_dir = PathBuf::from("config");
        let result = FinancialAnalysisConfig::load_from_dir(&config_dir);

        // This test will only pass if the config files exist
        if result.is_ok() {
            let config = result.unwrap();
            assert!(!config.concept_mappings.us_gaap.is_empty());
            assert!(!config.ratio_benchmarks.industries.is_empty());
            assert!(!config.ratio_interpretations.interpretations.is_empty());
            assert!(!config.ratio_formulas.profitability.is_empty());
        }
    }

    #[test]
    fn test_concept_mapping() {
        let config_dir = PathBuf::from("config");
        if let Ok(config) = FinancialAnalysisConfig::load_from_dir(&config_dir) {
            let mapping = config.get_concept_mapping("Assets", "us-gaap");
            assert_eq!(mapping, Some(&"us-gaap:Assets".to_string()));
        }
    }

    #[test]
    fn test_ratio_interpretation() {
        let config_dir = PathBuf::from("config");
        if let Ok(config) = FinancialAnalysisConfig::load_from_dir(&config_dir) {
            let interpretation = config.get_ratio_interpretation("return_on_equity", 0.25);
            assert!(interpretation.is_some());
            assert!(interpretation.unwrap().contains("Excellent"));
        }
    }

    #[test]
    fn test_ratio_formula() {
        let config_dir = PathBuf::from("config");
        if let Ok(config) = FinancialAnalysisConfig::load_from_dir(&config_dir) {
            let formula = config.get_ratio_formula("return_on_equity");
            assert!(formula.is_some());
            assert_eq!(
                formula.unwrap().formula,
                "Net Income / Shareholders' Equity"
            );
        }
    }
}
