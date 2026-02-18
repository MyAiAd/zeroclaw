//! Model Tier Configuration
//!
//! Central mapping of provider names and tier levels to OpenRouter model IDs.
//! Used by Telegram model switching and the native set_model_preference tool.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use std::sync::LazyLock;

/// Model reasoning tier levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelTier {
    Economy,
    Standard,
    High,
    Max,
}

impl fmt::Display for ModelTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelTier::Economy => write!(f, "economy"),
            ModelTier::Standard => write!(f, "standard"),
            ModelTier::High => write!(f, "high"),
            ModelTier::Max => write!(f, "max"),
        }
    }
}

impl FromStr for ModelTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "economy" | "eco" | "cheap" | "fast" => Ok(ModelTier::Economy),
            "standard" | "std" | "default" | "normal" => Ok(ModelTier::Standard),
            "high" | "hi" | "smart" | "advanced" => Ok(ModelTier::High),
            "max" | "maximum" | "best" | "top" => Ok(ModelTier::Max),
            _ => Err(format!("Unknown tier: {}", s)),
        }
    }
}

/// AI model providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelProvider {
    Anthropic,
    OpenAI,
    Google,
    DeepSeek,
}

impl fmt::Display for ModelProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelProvider::Anthropic => write!(f, "anthropic"),
            ModelProvider::OpenAI => write!(f, "openai"),
            ModelProvider::Google => write!(f, "google"),
            ModelProvider::DeepSeek => write!(f, "deepseek"),
        }
    }
}

impl FromStr for ModelProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Ok(ModelProvider::Anthropic),
            "openai" | "gpt" | "chatgpt" => Ok(ModelProvider::OpenAI),
            "google" | "gemini" => Ok(ModelProvider::Google),
            "deepseek" => Ok(ModelProvider::DeepSeek),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

/// A model entry with pricing information
#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub provider: ModelProvider,
    pub model_id: String,
    pub display_name: String,
    pub cost_per_1m: f64,
}

/// Provider aliases for natural language parsing
pub static PROVIDER_ALIASES: LazyLock<HashMap<&'static str, ModelProvider>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("claude", ModelProvider::Anthropic);
    m.insert("gpt", ModelProvider::OpenAI);
    m.insert("gemini", ModelProvider::Google);
    m
});

/// Model tier mappings
pub static MODEL_TIERS: LazyLock<HashMap<(ModelProvider, ModelTier), ModelEntry>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();

        // Economy tier
        m.insert(
            (ModelProvider::Anthropic, ModelTier::Economy),
            ModelEntry {
                provider: ModelProvider::Anthropic,
                model_id: "anthropic/claude-haiku-4.5".to_string(),
                display_name: "Claude Haiku 4.5".to_string(),
                cost_per_1m: 0.80,
            },
        );
        m.insert(
            (ModelProvider::OpenAI, ModelTier::Economy),
            ModelEntry {
                provider: ModelProvider::OpenAI,
                model_id: "openai/o4-mini".to_string(),
                display_name: "GPT-4o Mini".to_string(),
                cost_per_1m: 1.20,
            },
        );
        m.insert(
            (ModelProvider::DeepSeek, ModelTier::Economy),
            ModelEntry {
                provider: ModelProvider::DeepSeek,
                model_id: "deepseek/deepseek-v3.2".to_string(),
                display_name: "DeepSeek V3.2".to_string(),
                cost_per_1m: 0.14,
            },
        );

        // Standard tier
        m.insert(
            (ModelProvider::Anthropic, ModelTier::Standard),
            ModelEntry {
                provider: ModelProvider::Anthropic,
                model_id: "anthropic/claude-sonnet-4.5".to_string(),
                display_name: "Claude Sonnet 4.5".to_string(),
                cost_per_1m: 3.00,
            },
        );
        m.insert(
            (ModelProvider::OpenAI, ModelTier::Standard),
            ModelEntry {
                provider: ModelProvider::OpenAI,
                model_id: "openai/gpt-4.1".to_string(),
                display_name: "GPT-4.1".to_string(),
                cost_per_1m: 2.00,
            },
        );
        m.insert(
            (ModelProvider::Google, ModelTier::Standard),
            ModelEntry {
                provider: ModelProvider::Google,
                model_id: "google/gemini-2.5-flash".to_string(),
                display_name: "Gemini 2.5 Flash".to_string(),
                cost_per_1m: 0.75,
            },
        );

        // High tier
        m.insert(
            (ModelProvider::Anthropic, ModelTier::High),
            ModelEntry {
                provider: ModelProvider::Anthropic,
                model_id: "anthropic/claude-opus-4.5".to_string(),
                display_name: "Claude Opus 4.5".to_string(),
                cost_per_1m: 15.00,
            },
        );
        m.insert(
            (ModelProvider::OpenAI, ModelTier::High),
            ModelEntry {
                provider: ModelProvider::OpenAI,
                model_id: "openai/o3".to_string(),
                display_name: "o3".to_string(),
                cost_per_1m: 15.00,
            },
        );
        m.insert(
            (ModelProvider::Google, ModelTier::High),
            ModelEntry {
                provider: ModelProvider::Google,
                model_id: "google/gemini-3-pro-preview".to_string(),
                display_name: "Gemini 3 Pro".to_string(),
                cost_per_1m: 7.00,
            },
        );

        // Max tier
        m.insert(
            (ModelProvider::Anthropic, ModelTier::Max),
            ModelEntry {
                provider: ModelProvider::Anthropic,
                model_id: "anthropic/claude-opus-4.6".to_string(),
                display_name: "Claude Opus 4.6".to_string(),
                cost_per_1m: 20.00,
            },
        );
        m.insert(
            (ModelProvider::OpenAI, ModelTier::Max),
            ModelEntry {
                provider: ModelProvider::OpenAI,
                model_id: "openai/o3-pro".to_string(),
                display_name: "o3 Pro".to_string(),
                cost_per_1m: 60.00,
            },
        );

        m
    });

/// Get the model entry for a given provider and tier.
/// Returns None if the combination is not available.
pub fn get_model_for_tier(provider: ModelProvider, tier: ModelTier) -> Option<&'static ModelEntry> {
    MODEL_TIERS.get(&(provider, tier))
}

/// Get the default tier level.
pub fn get_default_tier() -> ModelTier {
    ModelTier::Standard
}

/// Resolve a provider alias to the canonical provider.
pub fn resolve_provider_alias(input: &str) -> Option<ModelProvider> {
    let normalized = input.to_lowercase();

    // Check direct provider name
    if let Ok(provider) = ModelProvider::from_str(&normalized) {
        return Some(provider);
    }

    // Check aliases
    PROVIDER_ALIASES.get(normalized.as_str()).copied()
}

/// Get all available tiers for a provider.
pub fn get_available_tiers_for_provider(provider: ModelProvider) -> Vec<ModelTier> {
    let tiers = [
        ModelTier::Economy,
        ModelTier::Standard,
        ModelTier::High,
        ModelTier::Max,
    ];
    tiers
        .into_iter()
        .filter(|&tier| MODEL_TIERS.contains_key(&(provider, tier)))
        .collect()
}

/// Get the highest available tier for a provider.
pub fn get_max_tier_for_provider(provider: ModelProvider) -> Option<ModelTier> {
    let tier_order = [
        ModelTier::Max,
        ModelTier::High,
        ModelTier::Standard,
        ModelTier::Economy,
    ];
    tier_order
        .into_iter()
        .find(|&tier| MODEL_TIERS.contains_key(&(provider, tier)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_tier_display_and_parse() {
        assert_eq!(ModelTier::Economy.to_string(), "economy");
        assert_eq!(ModelTier::Standard.to_string(), "standard");
        assert_eq!(ModelTier::High.to_string(), "high");
        assert_eq!(ModelTier::Max.to_string(), "max");

        assert_eq!(ModelTier::from_str("economy").unwrap(), ModelTier::Economy);
        assert_eq!(
            ModelTier::from_str("STANDARD").unwrap(),
            ModelTier::Standard
        );
        assert_eq!(ModelTier::from_str("high").unwrap(), ModelTier::High);
        assert_eq!(ModelTier::from_str("max").unwrap(), ModelTier::Max);

        // Aliases
        assert_eq!(ModelTier::from_str("cheap").unwrap(), ModelTier::Economy);
        assert_eq!(ModelTier::from_str("best").unwrap(), ModelTier::Max);
    }

    #[test]
    fn model_provider_display_and_parse() {
        assert_eq!(ModelProvider::Anthropic.to_string(), "anthropic");
        assert_eq!(ModelProvider::OpenAI.to_string(), "openai");
        assert_eq!(ModelProvider::Google.to_string(), "google");
        assert_eq!(ModelProvider::DeepSeek.to_string(), "deepseek");

        assert_eq!(
            ModelProvider::from_str("anthropic").unwrap(),
            ModelProvider::Anthropic
        );
        assert_eq!(
            ModelProvider::from_str("claude").unwrap(),
            ModelProvider::Anthropic
        );
        assert_eq!(
            ModelProvider::from_str("gpt").unwrap(),
            ModelProvider::OpenAI
        );
        assert_eq!(
            ModelProvider::from_str("gemini").unwrap(),
            ModelProvider::Google
        );
    }

    #[test]
    fn get_model_for_tier_returns_correct_model() {
        let entry = get_model_for_tier(ModelProvider::Anthropic, ModelTier::Standard).unwrap();
        assert_eq!(entry.model_id, "anthropic/claude-sonnet-4.5");
        assert_eq!(entry.display_name, "Claude Sonnet 4.5");
        assert_eq!(entry.cost_per_1m, 3.00);

        let entry = get_model_for_tier(ModelProvider::OpenAI, ModelTier::High).unwrap();
        assert_eq!(entry.model_id, "openai/o3");
    }

    #[test]
    fn get_model_for_tier_returns_none_for_missing() {
        // Google doesn't have economy tier
        assert!(get_model_for_tier(ModelProvider::Google, ModelTier::Economy).is_none());
        // DeepSeek only has economy
        assert!(get_model_for_tier(ModelProvider::DeepSeek, ModelTier::Max).is_none());
    }

    #[test]
    fn resolve_provider_alias_works() {
        assert_eq!(
            resolve_provider_alias("claude"),
            Some(ModelProvider::Anthropic)
        );
        assert_eq!(resolve_provider_alias("gpt"), Some(ModelProvider::OpenAI));
        assert_eq!(
            resolve_provider_alias("gemini"),
            Some(ModelProvider::Google)
        );
        assert_eq!(
            resolve_provider_alias("anthropic"),
            Some(ModelProvider::Anthropic)
        );
        assert!(resolve_provider_alias("unknown").is_none());
    }

    #[test]
    fn get_available_tiers_for_provider_works() {
        let tiers = get_available_tiers_for_provider(ModelProvider::Anthropic);
        assert!(tiers.contains(&ModelTier::Economy));
        assert!(tiers.contains(&ModelTier::Standard));
        assert!(tiers.contains(&ModelTier::High));
        assert!(tiers.contains(&ModelTier::Max));

        let tiers = get_available_tiers_for_provider(ModelProvider::DeepSeek);
        assert_eq!(tiers.len(), 1);
        assert!(tiers.contains(&ModelTier::Economy));
    }

    #[test]
    fn get_max_tier_for_provider_works() {
        assert_eq!(
            get_max_tier_for_provider(ModelProvider::Anthropic),
            Some(ModelTier::Max)
        );
        assert_eq!(
            get_max_tier_for_provider(ModelProvider::DeepSeek),
            Some(ModelTier::Economy)
        );
    }

    #[test]
    fn get_default_tier_is_standard() {
        assert_eq!(get_default_tier(), ModelTier::Standard);
    }
}
