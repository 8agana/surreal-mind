# Stateful Inner Voice Design v2 - Hardened with Safety Guardrails
**Date:** 2025-09-03
**Author:** CC (revised based on critical review)
**Purpose:** Replace echo-chamber with grounded, dialectic AI partner

## Core Philosophy
Inner voice is Gemini bringing its unique cognitive style to MY memories with **strict grounding requirements**. No hallucinations, no ungrounded synthesis - every response must cite sources.

## Critical Safety Requirements

### 1. Strict Grounding Contract
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct GroundedResponse {
    pub answer: String,
    pub sources: Vec<SourceCitation>,
    pub confidence: f32,          // 0.0-1.0 based on source coverage
    pub coverage_ratio: f32,      // Percent of answer grounded in sources
    pub refused: bool,            // True if insufficient grounding
    pub notes: Option<String>,    // Explanation for refusals
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceCitation {
    pub source_id: String,        // Thought/Memory ID
    pub source_type: SourceType,  // KG Memory, Thought, etc.
    pub span: String,             // Actual text supporting the claim
    pub relevance: f32,           // How well this supports the answer
}

// Minimum coverage ratio to accept response
const MIN_COVERAGE_RATIO: f32 = 0.6;
const MIN_SOURCES_REQUIRED: usize = 2;
```

### 2. Dimensional Hygiene
```rust
impl SurrealMindServer {
    async fn search_thoughts_grounded(&self, query: &str, limit: usize) -> Result<Vec<Thought>> {
        // ALWAYS filter by current embedding dimensions
        let active_dim = self.get_active_embedding_dim().await?;
        let active_provider = self.get_active_embedding_provider().await?;
        
        let sql = "SELECT * FROM thoughts 
                   WHERE embedding_dim = $dim 
                   AND embedding_provider = $provider
                   AND vector::similarity::cosine(embedding, $query_embedding) > $threshold
                   ORDER BY vector::similarity::cosine(embedding, $query_embedding) DESC
                   LIMIT $limit";
                   
        let query_embedding = self.embed_query(query, active_provider, active_dim).await?;
        
        // Stamp all metadata for audit trail
        self.db.query(sql)
            .bind(("dim", active_dim))
            .bind(("provider", active_provider))
            .bind(("query_embedding", query_embedding))
            .bind(("threshold", 0.7))
            .bind(("limit", limit))
            .await?
            .take(0)
    }
}
```

### 3. KG-Only Injection Policy
```rust
async fn build_grounded_context(
    &self,
    session: &InnerVoiceSession,
    query: &str,
    max_tokens: usize,
) -> Result<String> {
    let mut context = String::new();
    let mut token_count = 0;
    
    // 1. Rolling summary (if exists)
    if let Some(summary) = &session.rolling_summary {
        context.push_str(&format!("## Session Summary\n{}\n\n", summary.text));
        token_count += estimate_tokens(&summary.text);
    }
    
    // 2. Recent exchanges (token-budgeted)
    let recent_budget = max_tokens / 3;
    for exchange in session.context_window.iter().rev() {
        if token_count + estimate_tokens(&exchange.answer) > recent_budget {
            break;
        }
        context.push_str(&format!(
            "Previous: {}\nSynthesis: {}\n\n",
            exchange.my_query, exchange.answer
        ));
        token_count += estimate_tokens(&exchange.answer);
    }
    
    // 3. KG memories ONLY (no raw thoughts)
    let kg_budget = max_tokens - token_count;
    let memories = self.get_orbital_memories_grounded(2, kg_budget / 100).await?;
    
    context.push_str("## Relevant Knowledge\n");
    for memory in memories {
        let snippet = format!("[{}] {}\n", memory.id, memory.description);
        if token_count + estimate_tokens(&snippet) > max_tokens {
            break;
        }
        context.push_str(&snippet);
        token_count += estimate_tokens(&snippet);
    }
    
    context.push_str(&format!("\n## Current Question\n{}\n", query));
    context.push_str("\nProvide grounded synthesis with source citations. Use format: {\"answer\": \"...\", \"sources\": [{\"source_id\": \"...\", \"span\": \"...\"}]}");
    
    Ok(context)
}
```

### 4. Provider Abstraction
```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn synthesize(
        &self,
        prompt: &str,
        config: &LlmConfig,
    ) -> Result<GroundedResponse>;
    
    fn provider_name(&self) -> &str;
    fn max_context_tokens(&self) -> usize;
    fn cost_per_token(&self) -> f32;
}

pub struct GeminiClient {
    api_key: String,
    base_url: String,
}

#[async_trait]
impl LlmClient for GeminiClient {
    async fn synthesize(
        &self,
        prompt: &str,
        config: &LlmConfig,
    ) -> Result<GroundedResponse> {
        let start = Instant::now();
        
        // Call Gemini MCP tool
        let response = self.call_gemini_mcp(prompt, config).await?;
        
        // Parse response as JSON with strict schema
        let parsed: GroundedResponse = serde_json::from_str(&response)
            .map_err(|_| anyhow!("LLM response not in required grounded format"))?;
            
        // Validate grounding requirements
        if parsed.coverage_ratio < MIN_COVERAGE_RATIO || parsed.sources.len() < MIN_SOURCES_REQUIRED {
            return Ok(GroundedResponse {
                answer: "Insufficient grounding in available sources.".to_string(),
                sources: vec![],
                confidence: 0.0,
                coverage_ratio: 0.0,
                refused: true,
                notes: Some(format!(
                    "Required coverage: {}, actual: {}. Required sources: {}, actual: {}",
                    MIN_COVERAGE_RATIO, parsed.coverage_ratio,
                    MIN_SOURCES_REQUIRED, parsed.sources.len()
                )),
            });
        }
        
        // Log metrics (respects MCP_NO_LOG)
        self.log_metrics(&InnerVoiceMetrics {
            latency_ms: start.elapsed().as_millis() as u32,
            prompt_tokens: estimate_tokens(prompt),
            completion_tokens: estimate_tokens(&parsed.answer),
            coverage_ratio: parsed.coverage_ratio,
            refused: parsed.refused,
            cost_estimate: self.estimate_cost(prompt, &parsed.answer),
        }).await;
        
        Ok(parsed)
    }
}
```

### 5. Session Schema (Hardened)
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct InnerVoiceSession {
    pub id: String,
    pub version: u32,                    // Schema version for migrations
    pub provider: String,                // Which LLM provider
    pub model: String,                   // Specific model
    pub max_prompt_tokens: usize,        // Hard token budget
    pub rolling_summary: Option<RollingSummary>,
    pub context_window: Vec<DialogueExchange>,
    pub thread_topic: String,
    pub created_at: DateTime<Utc>,
    pub last_interaction: DateTime<Utc>,
    pub total_exchanges: u32,
    pub total_tokens: u64,               // Lifetime usage tracking
    pub total_cost: f32,                 // Lifetime cost tracking
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DialogueExchange {
    pub timestamp: DateTime<Utc>,
    pub my_query: String,
    pub answer: String,                  // From GroundedResponse.answer
    pub sources: Vec<SourceCitation>,    // From GroundedResponse.sources  
    pub confidence: f32,
    pub coverage_ratio: f32,
    pub refused: bool,
    pub memories_referenced: Vec<String>, // KG memory IDs used
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub latency_ms: u32,
    pub cost_estimate: f32,
    // NOTE: No full prompts stored (privacy)
    pub prompt_hash: String,             // For debugging without privacy leak
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RollingSummary {
    pub text: String,
    pub last_updated: DateTime<Utc>,
    pub exchanges_summarized: u32,
    pub token_savings: u32,              // Tokens saved vs keeping full history
}
```

### 6. Feature Flag & Configuration
```rust
// Environment configuration
pub struct InnerVoiceConfig {
    pub enabled: bool,                   // SURR_ENABLE_INNER_VOICE=1
    pub provider: String,                // SURR_INNER_VOICE_PROVIDER=gemini
    pub model: String,                   // SURR_INNER_VOICE_MODEL=gemini-2.5-pro
    pub max_prompt_tokens: usize,        // SURR_INNER_VOICE_MAX_TOKENS=8000
    pub temperature: f32,                // SURR_INNER_VOICE_TEMP=0.8
    pub min_coverage_ratio: f32,         // SURR_INNER_VOICE_MIN_COVERAGE=0.6
    pub session_timeout_hours: u32,      // SURR_INNER_VOICE_TIMEOUT=24
    pub enable_metrics: bool,            // SURR_INNER_VOICE_METRICS=1
    pub enable_cost_tracking: bool,      // SURR_INNER_VOICE_COST_TRACKING=1
}

impl Default for InnerVoiceConfig {
    fn default() -> Self {
        Self {
            enabled: false,              // DEFAULT OFF for safety
            provider: "gemini".to_string(),
            model: "gemini-2.5-pro".to_string(),
            max_prompt_tokens: 8000,
            temperature: 0.8,
            min_coverage_ratio: 0.6,
            session_timeout_hours: 24,
            enable_metrics: true,
            enable_cost_tracking: true,
        }
    }
}
```

### 7. Privacy & Observability
```rust
impl SurrealMindServer {
    async fn log_metrics(&self, metrics: &InnerVoiceMetrics) -> Result<()> {
        // Respect MCP_NO_LOG
        if env::var("MCP_NO_LOG").unwrap_or_default() == "1" {
            return Ok(());
        }
        
        // Only log aggregated metrics, never full prompts/responses
        info!(
            "inner_voice_call: latency={}ms, tokens={}+{}, coverage={:.2}, refused={}, cost=${:.4}",
            metrics.latency_ms,
            metrics.prompt_tokens,
            metrics.completion_tokens, 
            metrics.coverage_ratio,
            metrics.refused,
            metrics.cost_estimate
        );
        
        // Store in DB for prompt registry integration
        if self.config.enable_metrics {
            self.store_metrics(metrics).await?;
        }
        
        Ok(())
    }
}
```

### 8. Tool Interface (Hardened)
```rust
impl SurrealMindServer {
    pub async fn inner_voice(
        &self,
        query: &str,
        mode: InnerVoiceMode,
    ) -> Result<String> {
        // Feature gate
        if !self.inner_voice_config.enabled {
            return Err(anyhow!("inner_voice disabled. Set SURR_ENABLE_INNER_VOICE=1"));
        }
        
        // Load/create session
        let mut session = match mode {
            InnerVoiceMode::Continue => self.load_active_session().await?,
            InnerVoiceMode::NewThread(topic) => self.create_session(topic).await?,
        };
        
        // Build grounded context (token-budgeted, KG-only)
        let context = self.build_grounded_context(
            &session,
            query,
            self.inner_voice_config.max_prompt_tokens
        ).await?;
        
        // Call LLM with strict grounding requirements
        let llm_client = self.get_llm_client(&self.inner_voice_config.provider)?;
        let response = llm_client.synthesize(&context, &self.to_llm_config()).await?;
        
        // Handle refusals gracefully
        if response.refused {
            return Ok(format!(
                "I cannot provide a well-grounded answer to that question based on available sources. {}",
                response.notes.unwrap_or_default()
            ));
        }
        
        // Store exchange with full metadata
        let exchange = DialogueExchange {
            timestamp: Utc::now(),
            my_query: query.to_string(),
            answer: response.answer.clone(),
            sources: response.sources.clone(),
            confidence: response.confidence,
            coverage_ratio: response.coverage_ratio,
            refused: response.refused,
            memories_referenced: self.extract_memory_ids(&response.sources),
            prompt_tokens: estimate_tokens(&context) as u32,
            completion_tokens: estimate_tokens(&response.answer) as u32,
            latency_ms: 0, // Set by LlmClient
            cost_estimate: 0.0, // Set by LlmClient  
            prompt_hash: hash_string(&context),
        };
        
        session.context_window.push(exchange);
        session.last_interaction = Utc::now();
        session.total_exchanges += 1;
        
        // Rolling summary if window too large
        if session.context_window.len() > 10 {
            self.create_rolling_summary(&mut session).await?;
        }
        
        self.save_session(&session).await?;
        
        // Return formatted response with source attribution
        self.format_response_with_sources(&response)
    }
    
    fn format_response_with_sources(&self, response: &GroundedResponse) -> String {
        let mut formatted = response.answer.clone();
        
        if !response.sources.is_empty() {
            formatted.push_str("\n\n**Sources:**\n");
            for (i, source) in response.sources.iter().enumerate() {
                formatted.push_str(&format!(
                    "{}. [{}] {}\n",
                    i + 1,
                    source.source_id,
                    source.span
                ));
            }
        }
        
        if response.coverage_ratio < 0.8 {
            formatted.push_str(&format!(
                "\n*Note: Response based on {:.0}% source coverage*",
                response.coverage_ratio * 100.0
            ));
        }
        
        formatted
    }
}
```

## MVP Implementation Checklist

### Phase 1: Core Infrastructure
- [ ] `LlmClient` trait with Gemini implementation
- [ ] Hardened session schema with privacy defaults
- [ ] Token-budgeted context builder (KG-only)
- [ ] Feature flag system (`SURR_ENABLE_INNER_VOICE=1`)

### Phase 2: Grounding System
- [ ] `GroundedResponse` schema with validation
- [ ] Coverage ratio computation and gating
- [ ] Source citation requirement enforcement
- [ ] Refusal path for insufficient grounding

### Phase 3: Observability  
- [ ] Metrics logging (respects MCP_NO_LOG)
- [ ] Cost tracking and token budgets
- [ ] Prompt registry integration hooks
- [ ] Rolling summary system

### Phase 4: Safety & Testing
- [ ] Dimensional hygiene in all queries
- [ ] Privacy audit (no full prompt storage)
- [ ] Unit tests for grounding validation
- [ ] E2E test: refuse ungrounded responses

## Safety Guarantees

1. **No Hallucinations**: Every response requires source citations with coverage ≥60%
2. **Dimensional Safety**: All queries filter by active embedding dimensions
3. **KG-Only Injection**: No raw thoughts mixed into context, only processed memories
4. **Privacy First**: No full prompts stored, hash-only debugging
5. **Feature Gated**: Default OFF, explicit opt-in required
6. **Cost Control**: Token budgets and tracking prevent runaway costs
7. **Audit Trail**: All metrics logged (when MCP_NO_LOG≠1)

This design prevents the echo chamber disaster by requiring every statement to cite verifiable sources from the knowledge graph. No more pattern matching masquerading as intelligence.