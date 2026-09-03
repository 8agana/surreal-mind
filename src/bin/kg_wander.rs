//! kg_wander - Autonomous Knowledge Graph Explorer
//!
//! Uses the configured Google CLI provider to serendipitously explore the knowledge graph via the
//! `legacymind_wander` tool. It maintains a loop of:
//! 1. Observe current node
//! 2. Ask the provider "Where to next?"
//! 3. Execute wander step
//!
//! Run with: cargo run --bin kg_wander

use anyhow::Result;
use rmcp::model::CallToolRequestParams;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Write;
use surreal_mind::clients::{
    AntigravityClient, AntigravityPermissionMode, CognitiveAgent, GeminiClient, GoogleCliProvider,
};
use surreal_mind::config::Config;
use surreal_mind::server::SurrealMindServer;

fn bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(default)
}

const DEFAULT_MODEL: &str = "gemini-3-flash-preview";
const DEFAULT_MAX_STEPS: usize = 50;
const DEFAULT_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Serialize)]
struct AgentPrompt {
    current_node: Option<serde_json::Value>,
    affordances: Vec<String>,
    visited_count: usize,
    mission: String,
}

#[derive(Debug, Deserialize)]
struct AgentDecision {
    #[serde(default = "default_action")]
    action: String, // "wander", "connect", "create_entity", "observe"
    parameters: Option<serde_json::Value>,
    #[serde(default)]
    rationale: String,
}

fn default_action() -> String {
    "wander".to_string()
}

enum WanderDriver {
    Gemini(GeminiClient),
    Antigravity(AntigravityClient),
}

impl WanderDriver {
    async fn call(
        &self,
        prompt: &str,
    ) -> std::result::Result<surreal_mind::clients::AgentResponse, surreal_mind::clients::AgentError>
    {
        match self {
            Self::Gemini(client) => client.call(prompt, None).await,
            Self::Antigravity(client) => client.call(prompt, None).await,
        }
    }
}

fn kg_wander_model(provider: GoogleCliProvider, config: &Config) -> String {
    if let Ok(model) = std::env::var("KG_WANDER_MODEL") {
        let trimmed = model.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    match provider {
        GoogleCliProvider::Gemini => {
            std::env::var("GEMINI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
        }
        GoogleCliProvider::Antigravity => std::env::var("ANTIGRAVITY_MODEL")
            .or_else(|_| std::env::var("AGY_MODEL"))
            .unwrap_or_else(|_| config.system.antigravity_model.clone()),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env
    if dotenvy::dotenv().is_err() {
        // Ignore error, env might be set manually
    }

    println!("🚀 Starting kg_wander - Autonomous Gardener");

    if bool_env("DRY_RUN", false) {
        println!("[mode] DRY_RUN: wander skipped");
        return Ok(());
    }

    // Load config
    let config = Config::load().expect("Failed to load config");

    // Initialize Server (for tool execution)
    let server = SurrealMindServer::new(&config)
        .await
        .expect("Failed to start server");
    println!("✅ Connected to SurrealMind");

    // Initialize configured provider for decision making. Antigravity is the
    // default after KG quality signoff; Gemini remains available for rollback.
    let provider = GoogleCliProvider::from_env_or_config(Some(&config.system.google_cli_provider))
        .map_err(anyhow::Error::msg)?;
    let model = kg_wander_model(provider, &config);
    let timeout = std::env::var("KG_WANDER_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    println!("🧠 AI Driver: {} ({})", provider, model);

    let driver = match provider {
        GoogleCliProvider::Gemini => {
            WanderDriver::Gemini(GeminiClient::with_timeout_ms(model, timeout))
        }
        GoogleCliProvider::Antigravity => WanderDriver::Antigravity(
            AntigravityClient::new(Some(model))
                .with_timeout_ms(timeout)
                .with_print_timeout_ms(timeout)
                .with_permission_mode(AntigravityPermissionMode::for_kg()),
        ),
    };

    let max_steps = std::env::var("KG_WANDER_MAX_STEPS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_MAX_STEPS);

    // State
    let mut visited_ids: Vec<String> = Vec::new();
    let mut current_thought_id: Option<String> = None;
    let mut step_count = 0;

    // Initial wander (Random kick-off)
    println!("🎲 Initializing with random jump...");
    let initial_res = execute_wander(&server, "random", None, &visited_ids).await?;
    update_state(&initial_res, &mut current_thought_id, &mut visited_ids);
    print_node(&initial_res);

    // Refactored Loop
    let mut last_result = initial_res;

    loop {
        if step_count >= max_steps {
            println!("🛑 Max steps reached.");
            break;
        }

        step_count += 1;
        print!("\n[{}/{}] 🤔 Thinking... ", step_count, max_steps);
        std::io::stdout().flush()?;

        // 2. Ask configured provider
        let affordances = last_result["affordances"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|v| v.as_str().unwrap_or("unknown").to_string())
            .collect();

        let prompt_data = AgentPrompt {
            current_node: compact_current_node(&last_result["current_node"]),
            affordances,
            visited_count: visited_ids.len(),
            mission: "You are a Knowledge Gardener. Don't just wander! Actively build connections.\n\
                      ACTIONS:\n\
                      1. 'wander': { \"mode\": \"semantic\" | \"meta\" | \"random\" } - Move to a new node.\n\
                      2. 'connect': { \"target\": \"<node_id>\", \"rel_type\": \"related_to\" } - Connect current node to another.\n\
                      3. 'create_entity': { \"name\": \"...\", \"entity_type\": \"...\" } - Create a new concept related to this one.\n\
                      4. 'observe': { \"name\": \"Observation\", \"content\": \"...\" } - Add a note/observation.\n\
                      \n\
                      Prioritize 'wander' (semantic) usually, but randomly 'connect' or 'create_entity' if you spot missing links.".to_string(),
        };

        let prompt_str = format!(
            "You are a Knowledge Gardener.\n\
             Context: {}\n\n\
             Task: Decide your next move. Return ONLY valid JSON matching this schema.\n\
             CRITICAL INSTRUCTIONS:\n\
             1. Output raw JSON only. NO markdown blocks (```json). NO intro/outro text.\n\
             2. Do NOT write a summary or report. You are in a continuous loop.\n\
             3. Schema: {{ \"action\": \"wander\"|\"connect\"|\"create_entity\"|\"observe\", \"parameters\": {{...}}, \"rationale\": \"...\" }}",
            serde_json::to_string_pretty(&prompt_data)?
        );

        let decision_json = driver.call(&prompt_str).await?.response;
        let decision: AgentDecision = parse_json(&decision_json).unwrap_or_else(|| {
            println!("\n⚠️ Failed to parse: {}", decision_json);
            AgentDecision {
                action: "wander".to_string(),
                parameters: Some(json!({"mode": "random"})),
                rationale: "Failed to parse decision, defaulting to random wander.".to_string(),
            }
        });

        println!(
            "\r👉 {} ({})",
            decision.action.to_uppercase(),
            decision.rationale
        );

        // 3. Execute Action
        match decision.action.as_str() {
            "connect" => {
                let params = decision.parameters.clone().unwrap_or(json!({}));
                let target = params.get("target").and_then(|s| s.as_str()).unwrap_or("");
                let rel_type = params
                    .get("rel_type")
                    .and_then(|s| s.as_str())
                    .unwrap_or("related_to");

                if let Some(src) = &current_thought_id {
                    println!("🔗 Connecting {} -> {} ({})", src, target, rel_type);
                    if !target.is_empty() {
                        let args = json!({
                            "kind": "relationship",
                            "data": {
                                "source": src,
                                "target": target,
                                "rel_type": rel_type
                            }
                        });
                        let req = CallToolRequestParams::new("memories_create")
                            .with_arguments(args.as_object().unwrap().clone());
                        match server.handle_knowledgegraph_create(req).await {
                            Ok(_) => println!("✅ Connected!"),
                            Err(e) => println!("❌ Connect failed: {}", e),
                        }
                    }
                } else {
                    println!("❌ Cannot connect: No current node.");
                }
            }
            "create_entity" => {
                let params = decision.parameters.clone().unwrap_or(json!({}));
                let name = params.get("name").and_then(|s| s.as_str()).unwrap_or("");
                let etype = params
                    .get("entity_type")
                    .and_then(|s| s.as_str())
                    .unwrap_or("concept");

                if !name.is_empty() {
                    println!("✨ Creating Entity: {} ({})", name, etype);
                    let args = json!({
                        "kind": "entity",
                        "data": {
                            "name": name,
                            "entity_type": etype
                        }
                    });
                    let req = CallToolRequestParams::new("memories_create")
                        .with_arguments(args.as_object().unwrap().clone());
                    match server.handle_knowledgegraph_create(req).await {
                        Ok(_) => println!("✅ Created."),
                        Err(e) => println!("❌ Create failed: {}", e),
                    }
                }
            }
            "observe" => {
                let params = decision.parameters.clone().unwrap_or(json!({}));
                let name = params
                    .get("name")
                    .and_then(|s| s.as_str())
                    .unwrap_or("Observation");
                let content = params.get("content").and_then(|s| s.as_str()).unwrap_or("");

                if let Some(src) = &current_thought_id {
                    println!("📝 Observing on {}: {}", src, content);
                    let args = json!({
                        "kind": "observation",
                        "data": {
                            "name": name,
                            "source_thought_id": src,
                            "content": content
                        }
                    });
                    let req = CallToolRequestParams::new("memories_create")
                        .with_arguments(args.as_object().unwrap().clone());
                    match server.handle_knowledgegraph_create(req).await {
                        Ok(_) => println!("✅ Observed."),
                        Err(e) => println!("❌ Observe failed: {}", e),
                    }
                }
            }
            // Default to wander
            _ => {
                let params = decision.parameters.unwrap_or(json!({}));
                let mode = params
                    .get("mode")
                    .and_then(|s| s.as_str())
                    .unwrap_or("semantic");

                let cid = if mode == "random" {
                    None
                } else {
                    current_thought_id.clone()
                };

                match execute_wander(&server, mode, cid, &visited_ids).await {
                    Ok(res) => {
                        update_state(&res, &mut current_thought_id, &mut visited_ids);
                        print_node(&res);
                        last_result = res;
                    }
                    Err(e) => {
                        println!("❌ Wander failed: {}", e);
                        // Fallback to random if stuck
                        if mode != "random" {
                            println!("🔀 Fallback to random...");
                            if let Ok(res) =
                                execute_wander(&server, "random", None, &visited_ids).await
                            {
                                update_state(&res, &mut current_thought_id, &mut visited_ids);
                                print_node(&res);
                                last_result = res;
                            }
                        }
                    }
                }
            }
        }

        // Slight delay for readability
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    Ok(())
}

async fn execute_wander(
    server: &SurrealMindServer,
    mode: &str,
    current_thought_id: Option<String>,
    visited_ids: &[String],
) -> Result<serde_json::Value> {
    let params = json!({
        "mode": mode,
        "current_thought_id": current_thought_id,
        "visited_ids": visited_ids,
        "recency_bias": false
    });

    let req = CallToolRequestParams::new("legacymind_wander")
        .with_arguments(params.as_object().unwrap().clone());

    let result = server.handle_wander(req).await?;

    // Extract JSON content
    if let Some(content) = result.content.first() {
        if let rmcp::model::ContentBlock::Text(text) = content {
            let val: serde_json::Value = serde_json::from_str(&text.text)?;
            Ok(val)
        } else {
            Err(anyhow::anyhow!("Unexpected content type"))
        }
    } else {
        Err(anyhow::anyhow!("Empty response from wander tool"))
    }
}

fn update_state(
    res: &serde_json::Value,
    current_id: &mut Option<String>,
    visited: &mut Vec<String>,
) {
    if let Some(node) = res["current_node"].as_object()
        && let Some(id_val) = node.get("id")
        && let Some(id_str) = id_val.as_str()
    {
        *current_id = Some(id_str.to_string());
        visited.push(id_str.to_string());
    }
}

fn print_node(res: &serde_json::Value) {
    if let Some(node) = res["current_node"].as_object() {
        let content = node.get("content").and_then(|s| s.as_str()).unwrap_or(
            node.get("name")
                .and_then(|s| s.as_str())
                .unwrap_or("Unknown Node"),
        );
        let id = node.get("id").and_then(|s| s.as_str()).unwrap_or("?");

        println!(
            "📍 [{}] {}",
            id,
            content
                .chars()
                .take(100)
                .collect::<String>()
                .replace("\n", " ")
        );
    } else {
        println!("🌫️  Drifting... (No node found)");
    }
}

fn compact_current_node(node: &serde_json::Value) -> Option<serde_json::Value> {
    let object = node.as_object()?;
    let mut compact = serde_json::Map::new();

    for key in [
        "id",
        "table",
        "name",
        "entity_type",
        "created_at",
        "sim",
        "tags",
    ] {
        if let Some(value) = object.get(key) {
            compact.insert(key.to_string(), value.clone());
        }
    }

    if let Some(content) = object.get("content").and_then(|v| v.as_str()) {
        compact.insert(
            "content".to_string(),
            serde_json::Value::String(truncate_chars(content, 1_500)),
        );
    }

    Some(serde_json::Value::Object(compact))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn parse_json(s: &str) -> Option<AgentDecision> {
    // 1. Strip markdown fences if present
    let clean = s.trim();
    let clean = if clean.starts_with("```json") {
        clean
            .strip_prefix("```json")
            .unwrap_or(clean)
            .strip_suffix("```")
            .unwrap_or(clean)
    } else if clean.starts_with("```") {
        clean
            .strip_prefix("```")
            .unwrap_or(clean)
            .strip_suffix("```")
            .unwrap_or(clean)
    } else {
        clean
    };

    // 2. Heuristic find braces
    let start = clean.find('{')?;
    let end = clean.rfind('}')?;
    serde_json::from_str(&clean[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compact_current_node_drops_embedding_and_bounds_content() {
        let node = json!({
            "id": "thoughts:abc",
            "table": "thoughts",
            "content": "x".repeat(2_000),
            "embedding": [0.1, 0.2, 0.3],
            "data": {"large": "ignored"},
            "tags": ["continuity"],
            "sim": 0.82
        });

        let compact = compact_current_node(&node).expect("compact node");
        let object = compact.as_object().expect("compact object");

        assert_eq!(object.get("id"), Some(&json!("thoughts:abc")));
        assert_eq!(object.get("tags"), Some(&json!(["continuity"])));
        assert!(!object.contains_key("embedding"));
        assert!(!object.contains_key("data"));

        let content = object
            .get("content")
            .and_then(|v| v.as_str())
            .expect("content");
        assert_eq!(content.chars().count(), 1_503);
        assert!(content.ends_with("..."));
    }
}
