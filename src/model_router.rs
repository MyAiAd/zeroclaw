//! Model Router — Pre-routing for model switch detection
//!
//! Detects model-switch requests in user messages via regex patterns.
//! Used by Telegram channel and native set_model_preference tool.

use regex::Regex;
use std::sync::LazyLock;

use crate::model_tiers::{resolve_provider_alias, ModelProvider, ModelTier};

/// Persistence mode for model switches
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwitchPersistence {
    /// Keep this model for the rest of the session
    #[default]
    Sticky,
    /// Only use this model for the current message
    OneShot,
}

/// Result of regex-based model switch detection
#[derive(Debug, Clone)]
pub struct RegexDetectionResult {
    pub provider: Option<ModelProvider>,
    pub tier: Option<ModelTier>,
    pub persistence: SwitchPersistence,
    pub matched_pattern: String,
    pub matched_text: String,
}

// Regex patterns for model switch detection
static PROVIDER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:use|switch\s+to|change\s+to|swap\s+to|move\s+to|try)(?:\s+\w+){0,4}?\s+(claude|gpt|openai|anthropic|gemini|deepseek)\b")
        .expect("Invalid provider pattern")
});

static ESCALATION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:think\s+harder|smarter\s+model|more\s+reasoning|deep(?:er)?\s+analysis|your\s+(?:best|smartest|strongest)|absolute\s+best)\b")
        .expect("Invalid escalation pattern")
});

static DE_ESCALATION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:quick(?:er)?\s+(?:answer|response)|simpler\s+model|cheaper|faster)\b")
        .expect("Invalid de-escalation pattern")
});

static STICKY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:from\s+now\s+on|for\s+(?:the\s+)?rest|going\s+forward)\b")
        .expect("Invalid sticky pattern")
});

static ONE_SHOT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:just\s+(?:for\s+)?this|only\s+this\s+(?:one|time))\b")
        .expect("Invalid one-shot pattern")
});

static MAX_TIER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:absolute\s+best|your\s+(?:very\s+)?best|maximum|strongest|highest|most\s+(?:powerful|capable|intelligent|advanced)|top\s+(?:model|tier)|the\s+best\s+(?:model|one))\b")
        .expect("Invalid max tier pattern")
});

static HIGH_TIER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:think\s+harder|smarter|higher|better|more\s+(?:powerful|reasoning|advanced)|deep(?:er)?)\b")
        .expect("Invalid high tier pattern")
});

/// Patterns to strip from messages when extracting the clean question
static STRIP_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(?:use|switch\s+to|change\s+to|swap\s+to|move\s+to|try)(?:\s+\w+){0,4}?\s+(?:claude|gpt|openai|anthropic|gemini|deepseek)\s*").unwrap(),
        Regex::new(r"(?i)\b(?:think\s+harder|smarter\s+model|more\s+reasoning|deep(?:er)?\s+analysis)\s*").unwrap(),
        Regex::new(r"(?i)\b(?:higher|better|more\s+(?:powerful|advanced))\s+(?:model|tier)?\s*").unwrap(),
        Regex::new(r"(?i)\b(?:quick(?:er)?\s+(?:answer|response)|simpler\s+model|cheaper|faster)\s*").unwrap(),
        Regex::new(r"(?i)\b(?:from\s+now\s+on|for\s+(?:the\s+)?rest|going\s+forward)\s*").unwrap(),
        Regex::new(r"(?i)\b(?:just\s+(?:for\s+)?this|only\s+this\s+(?:one|time))\s*").unwrap(),
        Regex::new(r"(?i)\b(?:your\s+(?:very\s+)?(?:best|smartest|strongest)|absolute\s+best|maximum|strongest|highest|most\s+(?:powerful|capable|intelligent|advanced)|top\s+(?:model|tier)|the\s+best\s+(?:model|one))\s*").unwrap(),
        Regex::new(r"(?i)\bplease\s+use\s+a?\s*").unwrap(),
        Regex::new(r"(?i)\bcan\s+you\s+(?:use\s*|switch\s+to\s*)").unwrap(),
        Regex::new(r"(?i)\b(?:a\s+)?model\s+from\s+").unwrap(),
        Regex::new(r"(?i)\bto\s+(?:explain|analyze|help)").unwrap(),
    ]
});

/// Detect model switch intent using regex patterns.
/// Returns None if no switch detected.
pub fn detect_model_switch_regex(message: &str) -> Option<RegexDetectionResult> {
    let mut provider: Option<ModelProvider> = None;
    let mut tier: Option<ModelTier> = None;
    let mut persistence = SwitchPersistence::Sticky;
    let mut matched_pattern = String::new();
    let mut matched_text = String::new();

    // Check for explicit provider request
    if let Some(caps) = PROVIDER_PATTERN.captures(message) {
        let raw_provider = caps.get(1).map(|m| m.as_str().to_lowercase());
        if let Some(raw) = raw_provider {
            provider = resolve_provider_alias(&raw);
            matched_pattern = "provider".to_string();
            matched_text = caps
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        }
    }

    // Check for tier escalation
    if MAX_TIER_PATTERN.is_match(message) {
        tier = Some(ModelTier::Max);
        matched_pattern = if matched_pattern.is_empty() {
            "max".to_string()
        } else {
            format!("{}+max", matched_pattern)
        };
        if matched_text.is_empty() {
            if let Some(m) = MAX_TIER_PATTERN.find(message) {
                matched_text = m.as_str().to_string();
            }
        }
    } else if HIGH_TIER_PATTERN.is_match(message) {
        tier = Some(ModelTier::High);
        matched_pattern = if matched_pattern.is_empty() {
            "high".to_string()
        } else {
            format!("{}+high", matched_pattern)
        };
        if matched_text.is_empty() {
            if let Some(m) = HIGH_TIER_PATTERN.find(message) {
                matched_text = m.as_str().to_string();
            }
        }
    } else if ESCALATION_PATTERN.is_match(message) && tier.is_none() {
        tier = Some(ModelTier::High);
        matched_pattern = if matched_pattern.is_empty() {
            "escalation".to_string()
        } else {
            format!("{}+escalation", matched_pattern)
        };
        if matched_text.is_empty() {
            if let Some(m) = ESCALATION_PATTERN.find(message) {
                matched_text = m.as_str().to_string();
            }
        }
    }

    // Check for tier de-escalation
    if DE_ESCALATION_PATTERN.is_match(message) {
        tier = Some(ModelTier::Economy);
        matched_pattern = if matched_pattern.is_empty() {
            "economy".to_string()
        } else {
            format!("{}+economy", matched_pattern)
        };
        if matched_text.is_empty() {
            if let Some(m) = DE_ESCALATION_PATTERN.find(message) {
                matched_text = m.as_str().to_string();
            }
        }
    }

    // Check for persistence modifiers
    if ONE_SHOT_PATTERN.is_match(message) {
        persistence = SwitchPersistence::OneShot;
    }
    // Sticky is default, only override if one-shot not detected

    // Only return result if we detected something
    if provider.is_none() && tier.is_none() {
        return None;
    }

    Some(RegexDetectionResult {
        provider,
        tier,
        persistence,
        matched_pattern,
        matched_text,
    })
}

/// Extract the clean message by removing model-switch commands.
/// Returns None if the result is empty (switch-only command).
pub fn extract_clean_message(message: &str) -> Option<String> {
    let mut cleaned = message.to_string();

    for pattern in STRIP_PATTERNS.iter() {
        cleaned = pattern.replace_all(&cleaned, " ").to_string();
    }

    // Clean up extra whitespace and punctuation
    cleaned = cleaned
        .trim_start_matches(|c: char| c.is_whitespace() || ",.:;".contains(c))
        .trim_end_matches(|c: char| c.is_whitespace() || ",.:;".contains(c))
        .to_string();

    // Collapse multiple spaces
    let mut prev_space = false;
    cleaned = cleaned
        .chars()
        .filter(|&c| {
            if c == ' ' {
                if prev_space {
                    return false;
                }
                prev_space = true;
            } else {
                prev_space = false;
            }
            true
        })
        .collect();

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Generate notification text for a model switch.
pub fn generate_switch_notification(
    display_name: &str,
    persistence: SwitchPersistence,
    source: &str,
) -> String {
    if source == "ai" {
        return format!("*Escalated to {} for complex reasoning*\n\n", display_name);
    }

    match persistence {
        SwitchPersistence::OneShot => {
            format!("*Using {} for this response only*\n\n", display_name)
        }
        SwitchPersistence::Sticky => {
            format!("*Now using {} for this session*\n\n", display_name)
        }
    }
}

// -----------------------------------------------------------------------------
// Model Preference Storage (US-014)
// -----------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

/// Stored model preference for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreference {
    pub provider: String,
    pub model_id: String,
    pub tier: String,
    pub set_at: String,
    pub set_by: String, // "user" or "ai"
}

/// Memory key prefix for model preferences
const MODEL_PREF_KEY_PREFIX: &str = "system:model_preference";

/// Build the memory key for a session's model preference
pub fn model_pref_key(session_id: &str) -> String {
    format!("{}:{}", MODEL_PREF_KEY_PREFIX, session_id)
}

/// Get the current model preference for a session from memory.
/// Returns None if no preference is set.
pub async fn get_model_preference(
    memory: &dyn crate::memory::Memory,
    session_id: &str,
) -> Option<ModelPreference> {
    let key = model_pref_key(session_id);
    match memory.get(&key).await {
        Ok(Some(entry)) => serde_json::from_str(&entry.content).ok(),
        _ => None,
    }
}

/// Set the model preference for a session.
pub async fn set_model_preference(
    memory: &dyn crate::memory::Memory,
    session_id: &str,
    preference: &ModelPreference,
) -> anyhow::Result<()> {
    let key = model_pref_key(session_id);
    let content = serde_json::to_string(preference)?;
    memory
        .store(
            &key,
            &content,
            crate::memory::MemoryCategory::Custom("system".to_string()),
            Some(session_id),
        )
        .await
}

/// Clear the model preference for a session.
pub async fn clear_model_preference(
    memory: &dyn crate::memory::Memory,
    session_id: &str,
) -> anyhow::Result<bool> {
    let key = model_pref_key(session_id);
    memory.forget(&key).await
}

/// Create a new model preference from detection result.
pub fn create_model_preference(
    provider: ModelProvider,
    tier: ModelTier,
    model_id: &str,
    set_by: &str,
) -> ModelPreference {
    ModelPreference {
        provider: provider.to_string(),
        model_id: model_id.to_string(),
        tier: tier.to_string(),
        set_at: chrono::Utc::now().to_rfc3339(),
        set_by: set_by.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_tiers::{ModelProvider, ModelTier};
    use std::str::FromStr;

    #[test]
    fn detect_provider_request() {
        let result = detect_model_switch_regex("use claude for this").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::Anthropic));
        assert_eq!(result.matched_pattern, "provider");
    }

    #[test]
    fn detect_switch_to_provider() {
        let result = detect_model_switch_regex("switch to gpt please").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::OpenAI));
    }

    #[test]
    fn detect_tier_escalation() {
        let result = detect_model_switch_regex("think harder about this problem").unwrap();
        assert_eq!(result.tier, Some(ModelTier::High));
        assert!(result.matched_pattern.contains("high"));
    }

    #[test]
    fn detect_max_tier() {
        let result = detect_model_switch_regex("give me your absolute best answer").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Max));
    }

    #[test]
    fn detect_de_escalation() {
        let result = detect_model_switch_regex("just a quick answer please").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Economy));
    }

    #[test]
    fn detect_one_shot() {
        let result = detect_model_switch_regex("just for this one, use claude").unwrap();
        assert_eq!(result.persistence, SwitchPersistence::OneShot);
        assert_eq!(result.provider, Some(ModelProvider::Anthropic));
    }

    #[test]
    fn detect_combined_provider_and_tier() {
        let result = detect_model_switch_regex("use claude and think harder").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::Anthropic));
        assert_eq!(result.tier, Some(ModelTier::High));
        assert!(result.matched_pattern.contains("provider"));
        assert!(result.matched_pattern.contains("high"));
    }

    #[test]
    fn no_detection_for_normal_message() {
        let result = detect_model_switch_regex("What is the capital of France?");
        assert!(result.is_none());
    }

    #[test]
    fn extract_clean_message_removes_switch_commands() {
        // "use claude" is stripped, leaving "to explain quantum physics" -> "quantum physics"
        let cleaned = extract_clean_message("use claude to explain quantum physics").unwrap();
        assert_eq!(cleaned, "quantum physics");

        // Simple provider switch with question
        let cleaned2 =
            extract_clean_message("switch to gpt what is the capital of France?").unwrap();
        assert_eq!(cleaned2, "what is the capital of France?");
    }

    #[test]
    fn extract_clean_message_returns_none_for_switch_only() {
        let cleaned = extract_clean_message("use claude");
        assert!(cleaned.is_none());
    }

    #[test]
    fn generate_notification_sticky() {
        let notif =
            generate_switch_notification("Claude Opus 4.5", SwitchPersistence::Sticky, "user");
        assert!(notif.contains("Now using Claude Opus 4.5 for this session"));
    }

    #[test]
    fn generate_notification_one_shot() {
        let notif = generate_switch_notification("GPT-4.1", SwitchPersistence::OneShot, "user");
        assert!(notif.contains("Using GPT-4.1 for this response only"));
    }

    #[test]
    fn generate_notification_ai_escalation() {
        let notif =
            generate_switch_notification("Claude Opus 4.5", SwitchPersistence::Sticky, "ai");
        assert!(notif.contains("Escalated to Claude Opus 4.5 for complex reasoning"));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // US-028: Parity Tests (same 10 cases as TypeScript US-027)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn parity_01_use_claude_to_explain() {
        // "Use Claude to explain X" → switch to anthropic, cleaned: "explain X"
        let result = detect_model_switch_regex("Use Claude to explain quantum physics").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::Anthropic));
        assert_eq!(result.persistence, SwitchPersistence::Sticky);

        let cleaned = extract_clean_message("Use Claude to explain quantum physics").unwrap();
        assert_eq!(cleaned, "quantum physics");
    }

    #[test]
    fn parity_02_think_harder() {
        // "Think harder about this" → switch to high tier
        let result = detect_model_switch_regex("Think harder about this problem").unwrap();
        assert_eq!(result.tier, Some(ModelTier::High));
    }

    #[test]
    fn parity_03_switch_to_gpt_for_rest() {
        // "Switch to GPT for the rest" → openai, sticky
        let result =
            detect_model_switch_regex("Switch to GPT for the rest of our conversation").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::OpenAI));
        assert_eq!(result.persistence, SwitchPersistence::Sticky);
    }

    #[test]
    fn parity_04_just_use_gemini_this_one() {
        // "Just for this one, use Gemini" → google, one_shot
        let result = detect_model_switch_regex("Just for this one, use Gemini").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::Google));
        assert_eq!(result.persistence, SwitchPersistence::OneShot);
    }

    #[test]
    fn parity_05_whats_the_weather() {
        // "What's the weather?" → no switch
        let result = detect_model_switch_regex("What's the weather like today?");
        assert!(result.is_none());
    }

    #[test]
    fn parity_06_your_absolute_best() {
        // "Your absolute best" → max tier
        let result = detect_model_switch_regex("Give me your absolute best analysis").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Max));
    }

    #[test]
    fn parity_07_switch_to_claude_only() {
        // "Switch to Claude" → switchOnly (no question)
        let result = detect_model_switch_regex("Switch to Claude").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::Anthropic));

        let cleaned = extract_clean_message("Switch to Claude");
        assert!(cleaned.is_none()); // switch-only, no content
    }

    #[test]
    fn parity_08_use_cheaper_model() {
        // "Use a cheaper model" → economy
        let result = detect_model_switch_regex("Use a cheaper model for this").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Economy));
    }

    #[test]
    fn parity_09_faster_please() {
        // "Faster please" → economy
        let result = detect_model_switch_regex("Can you give me a faster response please").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Economy));
    }

    #[test]
    fn parity_10_switch_to_deepseek() {
        // "Switch to DeepSeek" → deepseek
        let result = detect_model_switch_regex("Switch to DeepSeek for this").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::DeepSeek));
    }

    // Additional tests for ModelTier and ModelProvider enum parsing
    #[test]
    fn model_tier_from_str_aliases() {
        assert_eq!(ModelTier::from_str("economy").unwrap(), ModelTier::Economy);
        assert_eq!(ModelTier::from_str("eco").unwrap(), ModelTier::Economy);
        assert_eq!(ModelTier::from_str("cheap").unwrap(), ModelTier::Economy);
        assert_eq!(ModelTier::from_str("fast").unwrap(), ModelTier::Economy);
        assert_eq!(
            ModelTier::from_str("standard").unwrap(),
            ModelTier::Standard
        );
        assert_eq!(ModelTier::from_str("std").unwrap(), ModelTier::Standard);
        assert_eq!(ModelTier::from_str("high").unwrap(), ModelTier::High);
        assert_eq!(ModelTier::from_str("smart").unwrap(), ModelTier::High);
        assert_eq!(ModelTier::from_str("max").unwrap(), ModelTier::Max);
        assert_eq!(ModelTier::from_str("best").unwrap(), ModelTier::Max);
    }

    #[test]
    fn model_provider_from_str_aliases() {
        assert_eq!(
            ModelProvider::from_str("anthropic").unwrap(),
            ModelProvider::Anthropic
        );
        assert_eq!(
            ModelProvider::from_str("claude").unwrap(),
            ModelProvider::Anthropic
        );
        assert_eq!(
            ModelProvider::from_str("openai").unwrap(),
            ModelProvider::OpenAI
        );
        assert_eq!(
            ModelProvider::from_str("gpt").unwrap(),
            ModelProvider::OpenAI
        );
        assert_eq!(
            ModelProvider::from_str("chatgpt").unwrap(),
            ModelProvider::OpenAI
        );
        assert_eq!(
            ModelProvider::from_str("google").unwrap(),
            ModelProvider::Google
        );
        assert_eq!(
            ModelProvider::from_str("gemini").unwrap(),
            ModelProvider::Google
        );
        assert_eq!(
            ModelProvider::from_str("deepseek").unwrap(),
            ModelProvider::DeepSeek
        );
    }

    // Expanded natural-language detection (Bug A fix)
    #[test]
    fn detect_switch_to_higher_model() {
        let result = detect_model_switch_regex("Can you switch to a higher model ?").unwrap();
        assert_eq!(result.tier, Some(ModelTier::High));
    }

    #[test]
    fn detect_change_to_model_from_claude() {
        let result = detect_model_switch_regex("Please change to a model from Claude").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::Anthropic));
    }

    #[test]
    fn detect_change_to_claude() {
        let result = detect_model_switch_regex("change to Claude for this").unwrap();
        assert_eq!(result.provider, Some(ModelProvider::Anthropic));
    }

    #[test]
    fn detect_better_model() {
        let result = detect_model_switch_regex("use a better model").unwrap();
        assert_eq!(result.tier, Some(ModelTier::High));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Regression tests: phrases that were previously not detected (Bug B fix)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn detect_highest_model() {
        let result = detect_model_switch_regex("switch to the highest model").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Max));
    }

    #[test]
    fn detect_most_powerful_model() {
        let result = detect_model_switch_regex("use your most powerful model").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Max));
    }

    #[test]
    fn detect_most_capable_model() {
        let result = detect_model_switch_regex("can you switch to the most capable model").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Max));
    }

    #[test]
    fn detect_top_model() {
        let result = detect_model_switch_regex("please use the top model").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Max));
    }

    #[test]
    fn detect_the_best_model() {
        let result = detect_model_switch_regex("use the best model for this").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Max));
    }

    #[test]
    fn detect_highest_defaults_to_anthropic() {
        // When no provider specified, should default to Anthropic max tier
        let result = detect_model_switch_regex("switch to the highest model").unwrap();
        assert_eq!(result.tier, Some(ModelTier::Max));
        assert!(result.provider.is_none()); // provider resolved by caller to Anthropic
    }
}
