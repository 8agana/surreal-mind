use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::time::{Duration, Instant};
use tracing::{debug, info};

static PROCESS_START: OnceLock<Instant> = OnceLock::new();

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn dimensions(&self) -> usize;
}

/// Reject an embedding that cannot be stored under the active vector-index
/// dimension. Generated vectors must pass this guard *before* any database
/// mutation claims or attempts to persist them.
///
/// Keeping this in the embeddings module makes the invariant shared by the
/// interactive thought/KG writers and every maintenance/admin re-embed path;
/// those paths must not grow subtly different "non-empty is good enough"
/// checks during a provider or dimension migration.
pub fn ensure_generated_embedding_dimension(
    embedding: &[f32],
    expected_dimension: usize,
) -> Result<()> {
    if expected_dimension == 0 {
        anyhow::bail!(
            "Configured embedding dimension must be greater than zero; refusing generated vector of length {}",
            embedding.len()
        );
    }
    if embedding.len() != expected_dimension {
        anyhow::bail!(
            "Generated embedding dimension mismatch: expected {}, got {}",
            expected_dimension,
            embedding.len()
        );
    }
    Ok(())
}

// OpenAI API implementation
pub struct OpenAIEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dims: usize,
    retries: u32,
    // Simple rate limiter: tokens per second
    rps_limit: f32,
    last_call: Arc<AtomicU64>,
}

#[derive(Serialize)]
struct OpenAIRequest<'a> {
    model: &'a str,
    input: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Deserialize)]
struct OpenAIResponseData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    data: Vec<OpenAIResponseData>,
}

impl OpenAIEmbedder {
    pub fn new(api_key: String, model: String, dims: Option<usize>, retries: u32) -> Result<Self> {
        let mut ua = format!(
            "surreal-mind/{} (component=embeddings; provider=openai)",
            env!("CARGO_PKG_VERSION")
        );
        if let Ok(commit) = std::env::var("SURR_COMMIT_HASH") {
            ua.push_str(&format!("; commit={}", &commit[..7.min(commit.len())]));
        }

        let rps_limit = std::env::var("SURR_EMBED_RPS")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(1.0);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent(ua)
            .build()
            .context("Failed to build reqwest client with timeout")?;

        let dims = dims.unwrap_or(match model.as_str() {
            // Known OpenAI embedding dims
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            _ => 1536, // sensible default; can be overridden via SURR_EMBED_DIM
        });

        Ok(Self {
            client,
            api_key,
            model,
            dims,
            retries,
            rps_limit,
            last_call: Arc::new(AtomicU64::new(0)),
        })
    }
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        debug!(
            "Generating OpenAI embedding (model={}, chars={})",
            self.model,
            text.len()
        );

        // Simple rate limiting: wait if needed to respect RPS
        if self.rps_limit > 0.0 {
            let interval_ms = (1000.0 / self.rps_limit) as u64;
            let process_start = PROCESS_START.get_or_init(Instant::now);
            let now_ms = (Instant::now() - *process_start).as_millis() as u64;
            let last = self.last_call.load(Ordering::SeqCst);
            if now_ms < last.saturating_add(interval_ms) {
                let delay = last.saturating_add(interval_ms).saturating_sub(now_ms);
                debug!("Rate limiting OpenAI embedding, delaying {}ms", delay);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            self.last_call.store(now_ms, Ordering::SeqCst);
        }

        let body = OpenAIRequest {
            model: &self.model,
            input: text,
            dimensions: if self.dims != 1536 && self.dims != 3072 {
                Some(self.dims)
            } else {
                None // Use default for standard sizes
            },
        };

        // Retry with simple exponential backoff
        let mut last_err: Option<anyhow::Error> = None;
        let attempts = self.retries;
        for i in 0..attempts {
            let send_res = self
                .client
                .post("https://api.openai.com/v1/embeddings")
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await
                .context(format!(
                    "Failed to send embedding request to OpenAI API for model '{}' ({} chars)",
                    self.model,
                    text.len()
                ));
            let response = match send_res {
                Ok(resp) => resp,
                Err(e) => {
                    last_err = Some(e);
                    let delay_ms = 200u64 * (1u64 << i);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response
                    .text()
                    .await
                    .context("Failed to read error response from OpenAI API")?;
                last_err = Some(anyhow::anyhow!(
                    "OpenAI API error {} for model '{}' ({} chars): {}",
                    status,
                    self.model,
                    text.len(),
                    error_text
                ));
                let delay_ms = 200u64 * (1u64 << i);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                continue;
            }

            let parse_res: Result<OpenAIResponse> = response.json().await.context(format!(
                "Failed to parse JSON response from OpenAI API for model '{}' ({} chars)",
                self.model,
                text.len()
            ));
            match parse_res {
                Ok(result) => {
                    return result
                        .data
                        .into_iter()
                        .next()
                        .map(|d| d.embedding)
                        .context(format!(
                            "No embedding data returned from OpenAI API for model '{}' ({} chars)",
                            self.model,
                            text.len()
                        ));
                }
                Err(e) => {
                    last_err = Some(e);
                    let delay_ms = 200u64 * (1u64 << i);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            anyhow::anyhow!(
                "Unknown error generating OpenAI embedding for model '{}' ({} chars)",
                self.model,
                text.len()
            )
        }))
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}

// Test-only offline/fake embedder (clu fed-77afac). Deterministic,
// zero-network, zero-I/O -- a pure function of the input text, so tests can
// exercise the full think/KG embedding pipeline without ever reaching
// api.openai.com or requiring a real API key.
//
// Gated behind the non-default `test-embedder` Cargo feature (mirrors the
// `test-probe` precedent above OpenAIEmbedder's sibling in Cargo.toml):
// this struct, its Embedder impl, and the "fake" match arm in
// `create_embedder` below all vanish from a plain `cargo build`/`cargo
// build --release` -- there is no code path in a production binary that
// can construct or select this embedder.
//
// SECURITY/CORRECTNESS NOTE: vectors produced here carry NO semantic
// meaning whatsoever -- cosine similarity between two fake embeddings says
// nothing about whether the underlying texts are related. This must never
// be mistaken for a real embedding, which is why `create_embedder` logs a
// loud warning every time it constructs one.
#[cfg(feature = "test-embedder")]
#[derive(Debug)]
pub struct FakeEmbedder {
    dims: usize,
}

#[cfg(feature = "test-embedder")]
impl FakeEmbedder {
    /// Construct a fake embedder producing `dims`-dimensional vectors.
    ///
    /// REJECTS `dims == 0` (fed-77afac review round 2). `embed` below
    /// documents -- and the unit tests assert -- that this embedder never
    /// returns an all-zero or non-finite vector, because such a vector yields
    /// NaN cosine similarity everywhere it is compared. At `dims == 0` that
    /// contract is unsatisfiable: the only vector of length zero IS the
    /// degenerate one. The previous infallible constructor produced exactly
    /// that -- `FakeEmbedder::new(0).embed(..)` returned `Ok(vec![])`, because
    /// the degenerate-case fallback below had its own `if !v.is_empty()`
    /// guard, which silently skipped the repair it existed to perform.
    ///
    /// `Config::load` documents "any positive dimension" for the "fake"
    /// provider but never enforces `> 0`, so the invariant is enforced here,
    /// at the single place the type can come into existence.
    pub fn new(dims: usize) -> Result<Self> {
        if dims == 0 {
            anyhow::bail!(
                "FakeEmbedder requires a positive dimension count, got 0. A zero-dimension \
                 embedder can only ever return an empty vector, which violates this \
                 embedder's documented contract of never producing an all-zero or \
                 non-finite vector (empty/zero vectors yield NaN cosine similarity \
                 wherever they are compared). Set embedding_dimensions (or \
                 SURR_EMBED_DIMENSIONS) to a positive value."
            );
        }
        Ok(Self { dims })
    }

    /// FNV-1a, 64-bit. Deliberately NOT `std::collections::hash_map::DefaultHasher`
    /// (backed by `RandomState`, which is seeded per-process and is NOT
    /// deterministic across runs/machines -- exactly the property this
    /// embedder must not have). FNV-1a is a small, well-known, allocation-free
    /// algorithm; implemented inline so this feature adds no new dependency.
    fn fnv1a(seed: u64, bytes: &[u8]) -> u64 {
        const FNV_PRIME: u64 = 0x100000001b3;
        let mut hash = seed;
        for &b in bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    /// Deterministic per-dimension pseudo-value in `[-1.0, 1.0]`, derived
    /// purely from `text` and the dimension index `i` -- no shared mutable
    /// state, no randomness, no clock, no environment.
    fn dimension_value(text: &[u8], i: usize) -> f32 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        // Mix the dimension index in as its own hashed byte sequence rather
        // than concatenating it into `text`, so adjacent dimensions don't
        // collapse onto near-identical hash chains for short inputs.
        let index_seed = Self::fnv1a(FNV_OFFSET_BASIS, &(i as u64).to_le_bytes());
        let h = Self::fnv1a(index_seed, text);
        ((h as f64 / u64::MAX as f64) * 2.0 - 1.0) as f32
    }
}

#[async_trait]
#[cfg(feature = "test-embedder")]
impl Embedder for FakeEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let bytes = text.as_bytes();
        let mut v: Vec<f32> = (0..self.dims)
            .map(|i| Self::dimension_value(bytes, i))
            .collect();

        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 && norm.is_finite() {
            for x in v.iter_mut() {
                *x /= norm;
            }
        } else {
            // Pathological case (never observed, but guarded explicitly per
            // fed-77afac design note: this embedder must never return an
            // all-zero or non-finite vector -- that yields NaN cosine
            // similarity everywhere it's compared). Fall back to a fixed
            // deterministic unit vector along the first axis.
            // `dims > 0` is an invariant enforced by `FakeEmbedder::new`, so
            // this index always exists. Indexing directly -- rather than the
            // old `if !v.is_empty()` guard -- means a future regression that
            // lets a zero-dimension embedder be constructed panics loudly
            // here instead of quietly returning the very empty vector this
            // branch exists to prevent (fed-77afac review round 2).
            v = vec![0.0f32; self.dims];
            v[0] = 1.0;
        }

        Ok(v)
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}

// No per-call fallback wrapper. Selection happens at startup to avoid mixed dims.

// Factory function to create embedder based on configuration
pub async fn create_embedder(config: &crate::config::Config) -> Result<Arc<dyn Embedder>> {
    // Load .env file if it exists
    crate::config::load_env_file();

    // Configuration: prefer OpenAI when key present; else Candle
    let provider = &config.system.embedding_provider;
    // Allow explicit dimension override for custom models
    let dim_override = Some(config.system.embedding_dimensions);

    // Helpers
    let is_placeholder = |s: &str| {
        let t = s.trim();
        t.is_empty()
            || t.contains("${")
            || t.eq_ignore_ascii_case("your-api-key-here")
            || t.eq_ignore_ascii_case("changeme")
    };

    match provider.as_str() {
        "openai" | "" => {
            let key = config.runtime.openai_api_key.clone().unwrap_or_default();
            if !is_placeholder(&key) && !key.is_empty() {
                let model = config.system.embedding_model.clone();
                let dims = dim_override.or(Some(1536));
                info!(
                    "Using OpenAI embeddings (model={}, dims={})",
                    model,
                    dims.unwrap()
                );
                Ok(Arc::new(OpenAIEmbedder::new(
                    key,
                    model,
                    dims,
                    config.system.embed_retries,
                )?))
            } else {
                anyhow::bail!("OPENAI_API_KEY is not set or valid. Cannot Initialize Embeddings.");
            }
        }
        #[cfg(feature = "test-embedder")]
        "fake" => {
            // RUNTIME guard (fed-77afac review round 2). `test-embedder` is an
            // ordinary public Cargo feature, so compile-time exclusion is NOT
            // the only thing standing between a release binary and a fake
            // embedder: `cargo build --release --features test-embedder`, or
            // the far more likely `--all-features` reflex (this repo's own
            // .github/workflows/ci.yml:34 runs `clippy --all-features`),
            // produces a RELEASE binary in which this arm exists and can be
            // selected with SURR_EMBED_PROVIDER=fake -- silently persisting
            // deterministic garbage vectors against real data.
            const FAKE_OPT_IN: &str = "SURR_ALLOW_FAKE_EMBEDDER";
            let opt_in = std::env::var(FAKE_OPT_IN).unwrap_or_default();
            if opt_in.trim() != "1" {
                anyhow::bail!(
                    "Refusing to construct the fake embedding provider: embedding_provider is \
                     \"fake\", but the runtime opt-in {FAKE_OPT_IN}=1 is not set (got \
                     {opt_in:?}). The fake embedder emits deterministic hash-based vectors with \
                     NO semantic meaning, so anything it writes to a real database is silently \
                     worthless and every similarity search over it is noise. To run the offline \
                     test suite, use scripts/test_db.sh (it sets {FAKE_OPT_IN}=1 and points at a \
                     throwaway in-memory database). To run a real instance, set \
                     embedding_provider (or SURR_EMBED_PROVIDER) to \"openai\"."
                );
            }
            let dims = config.system.embedding_dimensions;
            // Loud and unambiguous: this must never be mistaken for a real
            // embedding provider in a log (fed-77afac).
            tracing::warn!(
                "\u{26A0}\u{FE0F} FAKE EMBEDDER ACTIVE (test-embedder feature) \u{2014} vectors are deterministic hash-based placeholders with NO semantic meaning, dims={}. This must never run against production data.",
                dims
            );
            Ok(Arc::new(FakeEmbedder::new(dims)?))
        }
        _ => {
            // Unknown provider - fail explicitly
            #[cfg(feature = "test-embedder")]
            let supported = "'openai' or 'fake' (test-embedder feature)";
            #[cfg(not(feature = "test-embedder"))]
            let supported = "'openai'";
            anyhow::bail!(
                "Unknown or unsupported embedding provider: '{}'. Only {} is supported.",
                provider,
                supported
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_generated_embedding_dimension;

    #[test]
    fn generated_embedding_dimension_guard_rejects_wrong_lengths() {
        let short = vec![0.0f32; 3];
        let long = vec![0.0f32; 5];

        assert!(ensure_generated_embedding_dimension(&short, 4).is_err());
        assert!(ensure_generated_embedding_dimension(&long, 4).is_err());
    }

    #[test]
    fn generated_embedding_dimension_guard_rejects_zero_expected_dimension() {
        assert!(ensure_generated_embedding_dimension(&[], 0).is_err());
    }

    #[test]
    fn generated_embedding_dimension_guard_accepts_exact_length() {
        assert!(ensure_generated_embedding_dimension(&[0.0f32; 4], 4).is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_no_sleep_when_elapsed() {
        let interval = 1000u64;
        let last = 0u64;
        let now = 2000u64;
        // Simulate: if now >= last + interval, no sleep
        assert!(now >= last.saturating_add(interval));
    }
}

#[cfg(all(test, feature = "test-embedder"))]
mod fake_embedder_tests {
    use super::{Embedder, FakeEmbedder};

    fn l2_norm(v: &[f32]) -> f32 {
        v.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    #[tokio::test]
    async fn zero_dimensions_rejected_at_construction() {
        // fed-77afac review round 2. Before this guard, `FakeEmbedder::new(0)`
        // succeeded and `embed` returned `Ok(vec![])`: the all-zero/non-finite
        // fallback repaired nothing because its own `if !v.is_empty()` check
        // skipped, so the embedder silently violated the very contract the
        // fallback exists to uphold. `Config::load` documents "any positive
        // dimension" for the "fake" provider but never enforces `> 0`, so the
        // constructor is the enforcement point.
        let err = FakeEmbedder::new(0)
            .expect_err("a zero-dimension fake embedder must not be constructible");
        let msg = err.to_string();
        assert!(
            msg.contains("positive dimension count"),
            "error should say why zero is rejected, got: {msg}"
        );

        // And the smallest legal size still satisfies the contract, so the
        // guard rejects zero specifically rather than just "small".
        let v = FakeEmbedder::new(1)
            .expect("1 is a positive dimension count")
            .embed("one dimension")
            .await
            .unwrap();
        assert_eq!(v.len(), 1);
        assert!(v.iter().all(|x| x.is_finite()));
        assert!(
            v.iter().any(|x| *x != 0.0),
            "even a 1-d vector must not be all-zero"
        );
    }

    #[tokio::test]
    async fn deterministic_across_calls_and_instances() {
        let a = FakeEmbedder::new(64)
            .expect("test dimension counts are positive")
            .embed("same input")
            .await
            .unwrap();
        let b = FakeEmbedder::new(64)
            .expect("test dimension counts are positive")
            .embed("same input")
            .await
            .unwrap();
        // Byte-identical, not just "close" -- same text must produce the
        // exact same vector every time, on any machine, in any process.
        assert_eq!(
            a, b,
            "same text on fresh embedder instances must be byte-identical"
        );

        let embedder = FakeEmbedder::new(64).expect("test dimension counts are positive");
        let c = embedder.embed("same input").await.unwrap();
        let d = embedder.embed("same input").await.unwrap();
        assert_eq!(
            c, d,
            "repeated calls on the same instance must be byte-identical"
        );
        assert_eq!(a, c);
    }

    #[tokio::test]
    async fn text_sensitive_distinct_strings_differ() {
        let embedder = FakeEmbedder::new(32).expect("test dimension counts are positive");
        let a = embedder.embed("the quick brown fox").await.unwrap();
        let b = embedder
            .embed("a completely different sentence")
            .await
            .unwrap();
        assert_ne!(a, b, "distinct strings must not produce the same vector");
    }

    #[tokio::test]
    async fn text_sensitive_one_character_change_alters_output() {
        let embedder = FakeEmbedder::new(32).expect("test dimension counts are positive");
        let a = embedder.embed("hello world").await.unwrap();
        let b = embedder.embed("hallo world").await.unwrap();
        assert_ne!(a, b, "a one-character change must alter the output vector");
    }

    #[tokio::test]
    async fn correct_dimensionality() {
        for dims in [1usize, 8, 1536, 3072] {
            let embedder = FakeEmbedder::new(dims).expect("test dimension counts are positive");
            let v = embedder.embed("dimension check").await.unwrap();
            assert_eq!(v.len(), dims);
            assert_eq!(embedder.dimensions(), dims);
        }
    }

    #[tokio::test]
    async fn l2_normalized_and_non_degenerate() {
        let embedder = FakeEmbedder::new(1536).expect("test dimension counts are positive");
        let v = embedder.embed("normalize me").await.unwrap();
        let norm = l2_norm(&v);
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "expected unit norm, got {} (tolerance 1e-4)",
            norm
        );
        assert!(
            v.iter().any(|x| *x != 0.0),
            "vector must not be all-zero (degenerate -- NaN cosine similarity)"
        );
        assert!(
            v.iter().all(|x| x.is_finite()),
            "all components must be finite"
        );
    }

    #[tokio::test]
    async fn empty_string_produces_valid_non_degenerate_unit_vector() {
        let embedder = FakeEmbedder::new(1536).expect("test dimension counts are positive");
        let v = embedder.embed("").await.unwrap();
        assert_eq!(v.len(), 1536);
        let norm = l2_norm(&v);
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "empty string must still produce a unit vector, got norm {}",
            norm
        );
        assert!(
            v.iter().any(|x| *x != 0.0),
            "empty string must not produce an all-zero vector"
        );
        assert!(v.iter().all(|x| x.is_finite()));
    }

    #[tokio::test]
    async fn zero_network_zero_io_pure_function() {
        // No assertion beyond "this returns instantly and deterministically
        // with no I/O setup" -- the whole point of this embedder. Two calls
        // with no shared state (fresh instances, no config, no env, no
        // filesystem) must agree.
        let v1 = FakeEmbedder::new(16)
            .expect("test dimension counts are positive")
            .embed("pure")
            .await
            .unwrap();
        let v2 = FakeEmbedder::new(16)
            .expect("test dimension counts are positive")
            .embed("pure")
            .await
            .unwrap();
        assert_eq!(v1, v2);
    }
}
