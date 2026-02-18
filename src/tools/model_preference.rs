//! Model Preference Tool — AI-invoked model switching
//!
//! Allows the AI to change its own model preference during a conversation,
//! typically for self-escalation when more reasoning power is needed.

use super::traits::{Tool, ToolResult};
use crate::memory::{Memory, MemoryCategory};
use crate::model_router::{
    generate_switch_notification, model_pref_key, ModelPreference, SwitchPersistence,
};
use crate::model_tiers::{
    get_available_tiers_for_provider, get_max_tier_for_provider, get_model_for_tier, ModelProvider,
    ModelTier,
};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Tool for AI to change model preference during conversation
pub struct SetModelPreferenceTool {
    memory: Arc<dyn Memory>,
}

impl SetModelPreferenceTool {
    pub fn new(memory: Arc<dyn Memory>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl Tool for SetModelPreferenceTool {
    fn name(&self) -> &str {
        "set_model_preference"
    }

    fn description(&self) -> &str {
        "Change the AI model for this conversation. Use when you realize a task needs more \
         reasoning power (escalate to 'high' or 'max' tier) or when the user requests a specific \
         provider. Tier options: economy (fast/cheap), standard (default), high (complex reasoning), \
         max (deepest analysis). Provider options: anthropic, openai, google, deepseek."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The current session/conversation ID"
                },
                "tier": {
                    "type": "string",
                    "enum": ["economy", "standard", "high", "max"],
                    "description": "Model tier: economy (fast), standard (balanced), high (reasoning), max (deepest)"
                },
                "provider": {
                    "type": "string",
                    "enum": ["anthropic", "openai", "google", "deepseek"],
                    "description": "Model provider (optional, defaults to anthropic if not specified)"
                },
                "reason": {
                    "type": "string",
                    "description": "Why the model switch is needed (shown to user for transparency)"
                }
            },
            "required": ["session_id", "tier"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'session_id' parameter"))?;

        let tier_str = args
            .get("tier")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'tier' parameter"))?;

        let tier: ModelTier = tier_str
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid tier: {}", tier_str))?;

        // Provider is optional; default to Anthropic if not specified
        let provider: ModelProvider = args
            .get("provider")
            .and_then(|v| v.as_str())
            .map(|s| s.parse().unwrap_or(ModelProvider::Anthropic))
            .unwrap_or(ModelProvider::Anthropic);

        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("AI-initiated model switch for better task handling");

        // Look up the model entry for this provider/tier (US-022: cost-awareness)
        let (model_entry, actual_tier, fallback_note) = match get_model_for_tier(provider, tier) {
            Some(entry) => (entry, tier, None),
            None => {
                // Tier not available for this provider - try alternatives
                let available_tiers = get_available_tiers_for_provider(provider);
                let max_tier = get_max_tier_for_provider(provider);

                if let Some(max) = max_tier {
                    if let Some(fallback_entry) = get_model_for_tier(provider, max) {
                        let note = format!(
                            "Note: {} tier not available for {}. Using {} tier instead. \
                             Available tiers for {}: {}. \
                             Tip: OpenRouter provides access to all models via a single API key.",
                            tier,
                            provider,
                            max,
                            provider,
                            available_tiers
                                .iter()
                                .map(|t| t.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        (fallback_entry, max, Some(note))
                    } else {
                        // No tiers available at all - suggest alternatives
                        return Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!(
                                "No models available for {}. \
                                 Try a different provider (anthropic, openai, google) or use OpenRouter \
                                 which provides access to all models via a single API key.",
                                provider
                            )),
                        });
                    }
                } else {
                    // Provider has no tiers configured
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "Provider {} has no models configured. \
                             Available providers: anthropic (all tiers), openai (all tiers), \
                             google (standard/high), deepseek (economy only). \
                             Tip: OpenRouter provides access to all models via a single API key.",
                            provider
                        )),
                    });
                }
            }
        };

        // Create preference (inline to avoid cross-crate trait issues)
        let preference = ModelPreference {
            provider: provider.to_string(),
            model_id: model_entry.model_id.clone(),
            tier: actual_tier.to_string(),
            set_at: chrono::Utc::now().to_rfc3339(),
            set_by: "ai".to_string(),
        };

        // Store preference directly using Memory trait (avoid cross-crate function call)
        let key = model_pref_key(session_id);
        let content = serde_json::to_string(&preference)
            .map_err(|e| anyhow::anyhow!("Failed to serialize preference: {e}"))?;

        match self
            .memory
            .store(
                &key,
                &content,
                MemoryCategory::Custom("system".to_string()),
                Some(session_id),
            )
            .await
        {
            Ok(()) => {
                let notification = generate_switch_notification(
                    &model_entry.display_name,
                    SwitchPersistence::Sticky,
                    "ai",
                );

                // US-022: Include cost information in response
                let cost_note = format!(
                    "Note: {} costs ~${:.2}/1M tokens.",
                    model_entry.display_name, model_entry.cost_per_1m
                );

                let mut output = format!(
                    "{}Switched to {} ({} tier). Reason: {}\n{}",
                    notification, model_entry.display_name, actual_tier, reason, cost_note
                );

                // Add fallback note if we had to use a different tier
                if let Some(note) = fallback_note {
                    let _ = std::fmt::Write::write_str(&mut output, "\n");
                    let _ = std::fmt::Write::write_str(&mut output, &note);
                }

                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to set model preference: {e}")),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::SqliteMemory;
    use tempfile::TempDir;

    fn test_mem() -> (TempDir, Arc<dyn Memory>) {
        let tmp = TempDir::new().unwrap();
        let mem = SqliteMemory::new(tmp.path()).unwrap();
        (tmp, Arc::new(mem))
    }

    #[test]
    fn name_and_schema() {
        let (_tmp, mem) = test_mem();
        let tool = SetModelPreferenceTool::new(mem);
        assert_eq!(tool.name(), "set_model_preference");
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["session_id"].is_object());
        assert!(schema["properties"]["tier"].is_object());
        assert!(schema["properties"]["provider"].is_object());
    }

    #[tokio::test]
    async fn set_preference_to_high_tier() {
        let (_tmp, mem) = test_mem();
        let tool = SetModelPreferenceTool::new(mem.clone());
        let result = tool
            .execute(json!({
                "session_id": "test-session-123",
                "tier": "high",
                "reason": "Complex reasoning task detected"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("Claude Opus 4.5"));
        assert!(result.output.contains("high"));
        assert!(result.output.contains("Complex reasoning task detected"));
    }

    #[tokio::test]
    async fn set_preference_with_provider() {
        let (_tmp, mem) = test_mem();
        let tool = SetModelPreferenceTool::new(mem.clone());
        let result = tool
            .execute(json!({
                "session_id": "test-session-456",
                "tier": "standard",
                "provider": "openai"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("GPT-4.1"));
    }

    #[tokio::test]
    async fn set_preference_invalid_tier() {
        let (_tmp, mem) = test_mem();
        let tool = SetModelPreferenceTool::new(mem);
        let result = tool
            .execute(json!({
                "session_id": "test-session-789",
                "tier": "ultra" // Invalid tier
            }))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn set_preference_missing_session_id() {
        let (_tmp, mem) = test_mem();
        let tool = SetModelPreferenceTool::new(mem);
        let result = tool.execute(json!({"tier": "high"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn set_preference_missing_tier() {
        let (_tmp, mem) = test_mem();
        let tool = SetModelPreferenceTool::new(mem);
        let result = tool.execute(json!({"session_id": "test-session"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn set_preference_unavailable_combo_falls_back() {
        let (_tmp, mem) = test_mem();
        let tool = SetModelPreferenceTool::new(mem);
        // DeepSeek only has economy tier - should fallback to economy
        let result = tool
            .execute(json!({
                "session_id": "test-session",
                "tier": "max",
                "provider": "deepseek"
            }))
            .await
            .unwrap();

        // Should succeed with fallback to economy tier
        assert!(result.success);
        assert!(result.output.contains("DeepSeek"));
        assert!(result.output.contains("economy"));
        // Should mention the fallback
        assert!(result.output.contains("not available"));
    }

    #[tokio::test]
    async fn set_preference_includes_cost() {
        let (_tmp, mem) = test_mem();
        let tool = SetModelPreferenceTool::new(mem);
        let result = tool
            .execute(json!({
                "session_id": "test-session",
                "tier": "high",
                "provider": "anthropic"
            }))
            .await
            .unwrap();

        assert!(result.success);
        // US-022: Should include cost info (format: ~$15.00/1M tokens)
        assert!(result.output.contains("/1M tokens"));
        assert!(result.output.contains("costs ~$"));
    }
}
