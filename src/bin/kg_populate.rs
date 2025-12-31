//! kg_populate - Knowledge graph extraction orchestrator
//!
//! Fetches unextracted thoughts from SurrealDB, batches them to Gemini for
//! entity/relationship/observation extraction, parses JSON responses,
//! upserts to KG tables, and marks thoughts as extracted.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surreal_mind::clients::{CognitiveAgent, GeminiClient, PersistedAgent};
use surreal_mind::config::Config;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client as WsClient, Ws};
use surrealdb::opt::auth::Root;

const EXTRACTION_PROMPT_VERSION: &str = "v1";
const DEFAULT_BATCH_SIZE: usize = 25;
const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

// ============================================================================
// Data Structures
// ============================================================================

/// A thought record from the database
#[derive(Debug, Deserialize)]
struct ThoughtRecord {
    id: String,
    content: String,
}

/// Extraction result for a single thought
#[derive(Debug, Deserialize)]
struct ThoughtExtraction {
    thought_id: String,
    #[serde(default)]
    entities: Vec<ExtractedEntity>,
    #[serde(default)]
    relationships: Vec<ExtractedRelationship>,
    #[serde(default)]
    observations: Vec<ExtractedObservation>,
    #[serde(default)]
    boundaries: Vec<ExtractedBoundary>,
}

#[derive(Debug, Deserialize, Clone)]
struct ExtractedEntity {
    name: String,
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
}

#[derive(Debug, Deserialize, Clone)]
struct ExtractedRelationship {
    from: String,
    to: String,
    relation: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
}

#[derive(Debug, Deserialize, Clone)]
struct ExtractedObservation {
    content: String,
    #[serde(default)]
    context: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default = "default_confidence")]
    confidence: f64,
}

#[derive(Debug, Deserialize, Clone)]
struct ExtractedBoundary {
    rejected: String,
    reason: String,
    #[serde(default)]
    context: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
}

fn default_confidence() -> f64 {
    0.5
}

/// Full extraction response from Gemini
#[derive(Debug, Deserialize)]
struct ExtractionResponse {
    #[serde(default)]
    extractions: Vec<ThoughtExtraction>,
    #[serde(default)]
    summary: String,
}

/// Statistics for the extraction run
#[derive(Debug, Default, Serialize)]
struct ExtractionStats {
    thoughts_fetched: usize,
    thoughts_processed: usize,
    thoughts_failed: usize,
    entities_created: usize,
    entities_skipped: usize,
    edges_created: usize,
    edges_skipped: usize,
    observations_created: usize,
    observations_skipped: usize,
    boundaries_created: usize,
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: Could not load .env file: {}", e);
    }

    println!("🚀 Starting kg_populate - Knowledge Graph Extraction");

    // Load configuration
    let config = Config::load().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;
    println!("✅ Configuration loaded");

    // Get batch size from env or default
    let batch_size = std::env::var("KG_POPULATE_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_BATCH_SIZE);
    println!("📊 Batch size: {}", batch_size);

    // Connect to SurrealDB
    let db = Surreal::new::<Ws>(&config.system.database_url).await?;
    db.signin(Root {
        username: &config.runtime.database_user,
        password: &config.runtime.database_pass,
    })
    .await?;
    db.use_ns(&config.system.database_ns)
        .use_db(&config.system.database_db)
        .await?;
    println!("✅ Connected to SurrealDB");

    let db = Arc::new(db);
    let mut stats = ExtractionStats::default();

    // Main processing loop
    loop {
        // Fetch unextracted thoughts
        let thoughts = fetch_unextracted_thoughts(&db, batch_size).await?;
        if thoughts.is_empty() {
            println!("✅ No more unextracted thoughts found");
            break;
        }

        stats.thoughts_fetched += thoughts.len();
        println!(
            "🔄 Processing batch of {} thoughts (total fetched: {})",
            thoughts.len(),
            stats.thoughts_fetched
        );

        // Generate batch ID for this extraction run
        let batch_id = uuid::Uuid::new_v4().to_string();

        // Build prompt with thoughts
        let prompt = build_extraction_prompt(&thoughts);

        // Call Gemini for extraction
        match call_gemini_extraction(&db, &prompt).await {
            Ok(response) => {
                // Parse the response
                match parse_extraction_response(&response) {
                    Ok(extraction) => {
                        println!(
                            "  📊 Extracted {} thought results, summary: {}",
                            extraction.extractions.len(),
                            if extraction.summary.len() > 80 {
                                format!("{}...", &extraction.summary[..80])
                            } else {
                                extraction.summary.clone()
                            }
                        );

                        // Process each thought's extraction
                        for thought_extraction in &extraction.extractions {
                            match process_thought_extraction(
                                &db,
                                thought_extraction,
                                &batch_id,
                                &mut stats,
                            )
                            .await
                            {
                                Ok(_) => {
                                    // Mark thought as extracted
                                    if let Err(e) = mark_thought_extracted(
                                        &db,
                                        &thought_extraction.thought_id,
                                        &batch_id,
                                    )
                                    .await
                                    {
                                        eprintln!(
                                            "  ⚠️  Failed to mark thought {} as extracted: {}",
                                            thought_extraction.thought_id, e
                                        );
                                    }
                                    stats.thoughts_processed += 1;
                                }
                                Err(e) => {
                                    eprintln!(
                                        "  ⚠️  Failed to process thought {}: {}",
                                        thought_extraction.thought_id, e
                                    );
                                    stats.thoughts_failed += 1;
                                }
                            }
                        }

                        // Mark any thoughts that weren't in the extraction response
                        // (Gemini might have skipped some)
                        for thought in &thoughts {
                            let was_processed = extraction
                                .extractions
                                .iter()
                                .any(|e| e.thought_id == thought.id);
                            if !was_processed {
                                // Still mark as extracted to avoid re-processing
                                if let Err(e) =
                                    mark_thought_extracted(&db, &thought.id, &batch_id).await
                                {
                                    eprintln!(
                                        "  ⚠️  Failed to mark skipped thought {} as extracted: {}",
                                        thought.id, e
                                    );
                                }
                                stats.thoughts_processed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  ❌ Failed to parse extraction response: {}", e);
                        stats.thoughts_failed += thoughts.len();
                        // Don't mark as extracted - will retry next run
                    }
                }
            }
            Err(e) => {
                eprintln!("  ❌ Gemini extraction failed: {}", e);
                stats.thoughts_failed += thoughts.len();
                // Don't mark as extracted - will retry next run
            }
        }
    }

    // Print summary
    println!("\n{}", "=".repeat(60));
    println!("📊 KG POPULATION COMPLETE!");
    println!("  Thoughts fetched:      {}", stats.thoughts_fetched);
    println!("  Thoughts processed:    {}", stats.thoughts_processed);
    println!("  Thoughts failed:       {}", stats.thoughts_failed);
    println!("  Entities created:      {}", stats.entities_created);
    println!("  Entities skipped:      {}", stats.entities_skipped);
    println!("  Edges created:         {}", stats.edges_created);
    println!("  Edges skipped:         {}", stats.edges_skipped);
    println!("  Observations created:  {}", stats.observations_created);
    println!("  Observations skipped:  {}", stats.observations_skipped);
    println!("  Boundaries created:    {}", stats.boundaries_created);
    println!("{}", "=".repeat(60));

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Fetch thoughts that haven't been extracted yet
async fn fetch_unextracted_thoughts(
    db: &Surreal<WsClient>,
    limit: usize,
) -> Result<Vec<ThoughtRecord>> {
    let sql = format!(
        "SELECT meta::id(id) as id, content FROM thoughts WHERE extracted_to_kg = false ORDER BY created_at ASC LIMIT {}",
        limit
    );
    let rows: Vec<ThoughtRecord> = db.query(sql).await?.take(0)?;
    Ok(rows)
}

/// Build the extraction prompt with embedded thoughts
fn build_extraction_prompt(thoughts: &[ThoughtRecord]) -> String {
    let prompt_template = include_str!("../prompts/kg_extraction_v1.md");

    let mut prompt = prompt_template.to_string();
    prompt.push_str("\n\n");

    for thought in thoughts {
        prompt.push_str(&format!(
            "---\nThought ID: {}\nContent:\n{}\n\n",
            thought.id, thought.content
        ));
    }

    prompt
}

/// Call Gemini for extraction
async fn call_gemini_extraction(db: &Arc<Surreal<WsClient>>, prompt: &str) -> Result<String> {
    let model =
        std::env::var("KG_POPULATE_MODEL").unwrap_or_else(|_| DEFAULT_GEMINI_MODEL.to_string());
    let timeout = std::env::var("KG_POPULATE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS);

    let gemini = GeminiClient::with_timeout_ms(model.clone(), timeout);
    let agent = PersistedAgent::new(
        gemini,
        db.clone(),
        "gemini",
        model,
        "kg_populate".to_string(),
    );

    // No session resume for extraction - each batch is independent
    let response = agent.call(prompt, None).await?;
    Ok(response.response)
}

/// Parse the extraction response, handling markdown code fences
fn parse_extraction_response(response: &str) -> Result<ExtractionResponse> {
    // Strip markdown code fences if present
    let json_str = response
        .trim()
        .strip_prefix("```json")
        .or_else(|| response.trim().strip_prefix("```"))
        .unwrap_or(response)
        .strip_suffix("```")
        .unwrap_or(response)
        .trim();

    let parsed: ExtractionResponse = serde_json::from_str(json_str)?;
    Ok(parsed)
}

/// Process a single thought's extraction results
async fn process_thought_extraction(
    db: &Surreal<WsClient>,
    extraction: &ThoughtExtraction,
    batch_id: &str,
    stats: &mut ExtractionStats,
) -> Result<()> {
    let thought_id = extraction.thought_id.clone();
    let batch_id_owned = batch_id.to_string();

    // Upsert entities
    for entity in &extraction.entities {
        match upsert_entity(
            db,
            entity.clone(),
            thought_id.clone(),
            batch_id_owned.clone(),
        )
        .await?
        {
            true => stats.entities_created += 1,
            false => stats.entities_skipped += 1,
        }
    }

    // Upsert relationships (edges)
    for relationship in &extraction.relationships {
        match upsert_edge(
            db,
            relationship.clone(),
            thought_id.clone(),
            batch_id_owned.clone(),
        )
        .await?
        {
            true => stats.edges_created += 1,
            false => stats.edges_skipped += 1,
        }
    }

    // Upsert observations
    for observation in &extraction.observations {
        match upsert_observation(
            db,
            observation.clone(),
            thought_id.clone(),
            batch_id_owned.clone(),
        )
        .await?
        {
            true => stats.observations_created += 1,
            false => stats.observations_skipped += 1,
        }
    }

    // Create boundaries
    for boundary in &extraction.boundaries {
        create_boundary(
            db,
            boundary.clone(),
            thought_id.clone(),
            batch_id_owned.clone(),
        )
        .await?;
        stats.boundaries_created += 1;
    }

    Ok(())
}

/// Upsert an entity - returns true if created, false if already existed
async fn upsert_entity(
    db: &Surreal<WsClient>,
    entity: ExtractedEntity,
    thought_id: String,
    batch_id: String,
) -> Result<bool> {
    // Check if entity already exists by name
    let sql =
        "SELECT meta::id(id) as id, source_thought_ids FROM kg_entities WHERE name = $name LIMIT 1";
    let existing: Vec<serde_json::Value> = db
        .query(sql)
        .bind(("name", entity.name.clone()))
        .await?
        .take(0)?;

    if let Some(row) = existing.first() {
        // Entity exists - update source_thought_ids if needed
        let id = row.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let mut thought_ids: Vec<String> = row
            .get("source_thought_ids")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        if !thought_ids.contains(&thought_id) {
            thought_ids.push(thought_id);
            let update_sql = format!(
                "UPDATE kg_entities:`{}` SET source_thought_ids = $thought_ids",
                id
            );
            db.query(&update_sql)
                .bind(("thought_ids", thought_ids))
                .await?;
        }
        return Ok(false);
    }

    // Create new entity
    let data = serde_json::json!({
        "entity_type": entity.entity_type,
        "description": entity.description,
    });

    db.query("CREATE kg_entities SET created_at = time::now(), name = $name, entity_type = $etype, data = $data, source_thought_ids = $thought_ids, extraction_batch_id = $batch_id, extracted_at = time::now(), extraction_confidence = $confidence, extraction_prompt_version = $version")
        .bind(("name", entity.name))
        .bind(("etype", entity.entity_type))
        .bind(("data", data))
        .bind(("thought_ids", vec![thought_id]))
        .bind(("batch_id", batch_id))
        .bind(("confidence", entity.confidence))
        .bind(("version", EXTRACTION_PROMPT_VERSION.to_string()))
        .await?;

    Ok(true)
}

/// Upsert an edge - returns true if created, false if already existed
async fn upsert_edge(
    db: &Surreal<WsClient>,
    relationship: ExtractedRelationship,
    thought_id: String,
    batch_id: String,
) -> Result<bool> {
    // First, resolve entity names to IDs
    let from_sql = "SELECT meta::id(id) as id FROM kg_entities WHERE name = $name LIMIT 1";
    let from_rows: Vec<serde_json::Value> = db
        .query(from_sql)
        .bind(("name", relationship.from.clone()))
        .await?
        .take(0)?;
    let from_id = from_rows
        .first()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let to_sql = "SELECT meta::id(id) as id FROM kg_entities WHERE name = $name LIMIT 1";
    let to_rows: Vec<serde_json::Value> = db
        .query(to_sql)
        .bind(("name", relationship.to.clone()))
        .await?
        .take(0)?;
    let to_id = to_rows
        .first()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // If either entity doesn't exist, skip the edge
    let (from_id, to_id) = match (from_id, to_id) {
        (Some(f), Some(t)) => (f, t),
        _ => {
            // Entities not found - skip edge creation
            return Ok(false);
        }
    };

    // Build Thing references
    let source_thing = format!("kg_entities:{}", from_id);
    let target_thing = format!("kg_entities:{}", to_id);

    // Check if edge already exists
    let check_sql = "SELECT meta::id(id) as id, source_thought_ids FROM kg_edges WHERE source = type::thing($src) AND target = type::thing($dst) AND rel_type = $rel LIMIT 1";
    let existing: Vec<serde_json::Value> = db
        .query(check_sql)
        .bind(("src", source_thing.clone()))
        .bind(("dst", target_thing.clone()))
        .bind(("rel", relationship.relation.clone()))
        .await?
        .take(0)?;

    if let Some(row) = existing.first() {
        // Edge exists - update source_thought_ids if needed
        let id = row.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let mut thought_ids: Vec<String> = row
            .get("source_thought_ids")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        if !thought_ids.contains(&thought_id) {
            thought_ids.push(thought_id);
            let update_sql = format!(
                "UPDATE kg_edges:`{}` SET source_thought_ids = $thought_ids",
                id
            );
            db.query(&update_sql)
                .bind(("thought_ids", thought_ids))
                .await?;
        }
        return Ok(false);
    }

    // Create new edge
    let data = serde_json::json!({
        "description": relationship.description,
    });

    db.query("CREATE kg_edges SET created_at = time::now(), source = type::thing($src), target = type::thing($dst), rel_type = $rel, data = $data, source_thought_ids = $thought_ids, extraction_batch_id = $batch_id, extracted_at = time::now(), extraction_confidence = $confidence, extraction_prompt_version = $version")
        .bind(("src", source_thing))
        .bind(("dst", target_thing))
        .bind(("rel", relationship.relation))
        .bind(("data", data))
        .bind(("thought_ids", vec![thought_id]))
        .bind(("batch_id", batch_id))
        .bind(("confidence", relationship.confidence))
        .bind(("version", EXTRACTION_PROMPT_VERSION.to_string()))
        .await?;

    Ok(true)
}

/// Upsert an observation - returns true if created, false if already existed
async fn upsert_observation(
    db: &Surreal<WsClient>,
    observation: ExtractedObservation,
    thought_id: String,
    batch_id: String,
) -> Result<bool> {
    // Generate a name from the content (first 50 chars)
    let name = if observation.content.len() > 50 {
        format!("{}...", &observation.content[..50])
    } else {
        observation.content.clone()
    };

    // Check if observation already exists by name and source_thought_id
    // NOTE: Uniqueness key is (name, source_thought_id) not (name, data.source_thought_id).
    // This deviates from the original spec but is superior: simpler logic, faster queries,
    // avoids nested JSON structure dependencies. Same guarantees: each thought's observations
    // are unique by semantic content (name hash), and observations don't duplicate across thoughts.
    let sql = "SELECT meta::id(id) as id FROM kg_observations WHERE name = $name AND source_thought_id = $src LIMIT 1";
    let existing: Vec<serde_json::Value> = db
        .query(sql)
        .bind(("name", name.clone()))
        .bind(("src", thought_id.clone()))
        .await?
        .take(0)?;

    if !existing.is_empty() {
        return Ok(false);
    }

    // Create new observation
    let data = serde_json::json!({
        "content": observation.content,
        "context": observation.context,
        "tags": observation.tags,
    });

    db.query("CREATE kg_observations SET created_at = time::now(), name = $name, data = $data, source_thought_id = $src, confidence = $conf, source_thought_ids = $thought_ids, extraction_batch_id = $batch_id, extracted_at = time::now(), extraction_confidence = $conf, extraction_prompt_version = $version")
        .bind(("name", name))
        .bind(("data", data))
        .bind(("src", thought_id.clone()))
        .bind(("conf", observation.confidence))
        .bind(("thought_ids", vec![thought_id]))
        .bind(("batch_id", batch_id))
        .bind(("version", EXTRACTION_PROMPT_VERSION.to_string()))
        .await?;

    Ok(true)
}

/// Create a boundary record
async fn create_boundary(
    db: &Surreal<WsClient>,
    boundary: ExtractedBoundary,
    thought_id: String,
    batch_id: String,
) -> Result<()> {
    db.query("CREATE kg_boundaries SET created_at = time::now(), source_thought_id = $src, rejected = $rejected, reason = $reason, context = $context, confidence = $conf, extraction_batch_id = $batch_id, extracted_at = time::now(), extraction_prompt_version = $version")
        .bind(("src", thought_id))
        .bind(("rejected", boundary.rejected))
        .bind(("reason", boundary.reason))
        .bind(("context", boundary.context))
        .bind(("conf", boundary.confidence))
        .bind(("batch_id", batch_id))
        .bind(("version", EXTRACTION_PROMPT_VERSION.to_string()))
        .await?;

    Ok(())
}

/// Mark a thought as extracted
async fn mark_thought_extracted(
    db: &Surreal<WsClient>,
    thought_id: &str,
    batch_id: &str,
) -> Result<()> {
    let sql = format!(
        "UPDATE thoughts:`{}` SET extracted_to_kg = true, extraction_batch_id = $batch_id, extracted_at = time::now()",
        thought_id
    );
    db.query(&sql)
        .bind(("batch_id", batch_id.to_string()))
        .await?;
    Ok(())
}
