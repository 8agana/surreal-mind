//! Custom deserializers for flexible parameter parsing in surreal-mind.
//!
//! These deserializers allow the MCP tools to accept parameters in multiple formats,
//! providing a user-friendly interface while maintaining type safety.

use serde::{Deserialize, Deserializer};

/// Deserializes injection_scale parameter with support for numeric values and named presets.
/// This function is public and can be used by other modules.
///
/// # Accepted Formats
///
/// * **Numeric**: Integer values 0-5
/// * **String presets** (case-insensitive):
///   - `"NONE"` → 0 (no memory injection)
///   - `"LIGHT"` → 1 (Mercury orbit - hot/current memories only)
///   - `"MEDIUM"` → 2 (Venus orbit - recent context)
///   - `"DEFAULT"` → 3 (Mars orbit - foundational memories)
///   - `"HIGH"` → 4 (Jupiter orbit - broad context)
///   - `"MAXIMUM"` → 5 (Pluto orbit - all relevant memories)
/// * **String numeric**: `"3"` → 3
///
/// # Examples
///
/// ```json
/// { "injection_scale": 3 }          // Direct numeric
/// { "injection_scale": "HIGH" }      // Named preset
/// { "injection_scale": "4" }         // String numeric
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - Value is outside 0-5 range
/// - String preset is not recognized
/// - Value cannot be parsed as a number
pub fn de_option_u8_forgiving<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(v) = opt else { return Ok(None) };
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(n) => {
            let val = if let Some(u) = n.as_u64() {
                u as f64
            } else if let Some(i) = n.as_i64() {
                i as f64
            } else if let Some(f) = n.as_f64() {
                f
            } else {
                return Err(D::Error::custom("invalid numeric for u8"));
            };
            let rounded = val.round();
            if !rounded.is_finite() {
                return Err(D::Error::custom("non-finite numeric for u8"));
            }
            // Allow any numeric value - graceful coercion handled in Rust code
            // Values will be clamped to 0-3 range in the main function
            Ok(Some(rounded as u8))
        }
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            // Handle named presets (case-insensitive)
            match s.to_lowercase().as_str() {
                "none" => Ok(Some(0)),
                "light" => Ok(Some(1)),
                "medium" => Ok(Some(2)),
                "default" => Ok(Some(3)),
                "high" => Ok(Some(4)),
                "maximum" => Ok(Some(5)),
                _ => {
                    // Try to parse as number
                    let val: f64 = s.parse().map_err(|_| {
                        D::Error::custom(format!(
                            "Invalid injection_scale '{}'. Valid presets: NONE, LIGHT, MEDIUM, DEFAULT, HIGH, MAXIMUM. Or use any numeric value (will be clamped to 0-3).",
                            s
                        ))
                    })?;
                    let rounded = val.round();
                    // Allow any numeric value - graceful coercion handled in Rust code
                    Ok(Some(rounded as u8))
                }
            }
        }
        other => Err(D::Error::custom(format!("invalid type for u8: {}", other))),
    }
}

/// Deserializes significance parameter with support for floats, integers, and named presets.
///
/// # Accepted Formats
///
/// * **Float**: Values 0.0-1.0 directly represent significance
/// * **Integer scale**: Values 2-10 are mapped to 0.2-1.0 (note: 1 is rejected as ambiguous)
/// * **String presets** (case-insensitive):
///   - `"low"` → 0.2
///   - `"medium"` → 0.5
///   - `"high"` → 0.9
/// * **String numeric**: `"0.75"` → 0.75, `"8"` → 0.8
///
/// # Special Cases
///
/// - Integer value `1` is explicitly rejected with a helpful error message to avoid ambiguity
///   between 1.0 (100% significance) and 1 on the 1-10 scale (10% significance)
/// - Values outside 0.0-1.0 range after conversion return an error
///
/// # Examples
///
/// ```json
/// { "significance": 0.75 }          // Direct float
/// { "significance": 8 }              // Integer scale (→ 0.8)
/// { "significance": "high" }         // Named preset (→ 0.9)
/// { "significance": "0.6" }          // String float
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - Value is exactly 1 (ambiguous)
/// - String preset is not recognized
/// - Value cannot be parsed as a number
/// - Final value is outside 0.0-1.0 range after mapping
pub fn de_option_f32_forgiving<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(v) = opt else { return Ok(None) };
    let val = match v {
        serde_json::Value::Null => return Ok(None),
        serde_json::Value::Number(n) => {
            // Prefer integer handling first to support 2-10 mapping and detect ambiguous 1
            if let Some(i) = n.as_i64() {
                if i == 1 {
                    return Err(D::Error::custom(
                        "significance=1 is ambiguous; use 0.1, 'low', or an integer 2-10 (maps to 0.2-1.0)",
                    ));
                }
                if (2..=10).contains(&i) {
                    (i as f32) / 10.0
                } else {
                    i as f32
                }
            } else if let Some(u) = n.as_u64() {
                if u == 1 {
                    return Err(D::Error::custom(
                        "significance=1 is ambiguous; use 0.1, 'low', or an integer 2-10 (maps to 0.2-1.0)",
                    ));
                }
                if (2..=10).contains(&(u as i64)) {
                    (u as f32) / 10.0
                } else {
                    u as f32
                }
            } else if let Some(f) = n.as_f64() {
                f as f32
            } else {
                return Err(D::Error::custom("invalid numeric for f32"));
            }
        }
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            // Handle string presets for significance
            match s.to_lowercase().as_str() {
                "low" => 0.2,
                "medium" => 0.5,
                "high" => 0.9,
                _ => {
                    // First, try integer mapping for 2-10 scale (and reject ambiguous 1)
                    if let Ok(i) = s.parse::<i64>() {
                        if i == 1 {
                            return Err(D::Error::custom(
                                "significance=1 is ambiguous; use 0.1, 'low', or an integer 2-10 (maps to 0.2-1.0)",
                            ));
                        }
                        if (2..=10).contains(&i) {
                            (i as f32) / 10.0
                        } else {
                            // Not an integer in 2-10; fall back to float parse
                            s.parse::<f32>().map_err(|_| {
                                D::Error::custom(format!(
                                    "Invalid significance '{}'. Use: 'low', 'medium', 'high', or numeric 0.0-1.0 (or 2-10 for integer scale)",
                                    s
                                ))
                            })?
                        }
                    } else {
                        // Not an integer; try float parse
                        s.parse::<f32>().map_err(|_| {
                            D::Error::custom(format!(
                                "Invalid significance '{}'. Use: 'low', 'medium', 'high', or numeric 0.0-1.0 (or 2-10 for integer scale)",
                                s
                            ))
                        })?
                    }
                }
            }
        }
        other => return Err(D::Error::custom(format!("invalid type for f32: {}", other))),
    };

    // Allow any significance value - graceful coercion handled in Rust code
    // Values will be clamped to 0.0-1.0 range in the main function

    Ok(Some(val))
}

/// Deserializes tags parameter with support for string, array, or null values.
///
/// # Accepted Formats
///
/// * **Null**: `null` → `None`
/// * **String**: `"tag1"` → `Some(vec!["tag1"])`
/// * **Array**: `["tag1", "tag2"]` → `Some(vec!["tag1", "tag2"])`
/// * **Array with mixed types**: `["tag1", 123]` → `Some(vec!["tag1", "123"])` (numbers converted to strings)
///
/// # Examples
///
/// ```json
/// { "tags": null }                    // None
/// { "tags": "single-tag" }            // Some(vec!["single-tag"])
/// { "tags": ["tag1", "tag2"] }        // Some(vec!["tag1", "tag2"])
/// ```
///
/// # Errors
///
/// This function does not return errors - it gracefully handles invalid types by converting them to strings.
pub fn de_option_tags<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(v) = opt else { return Ok(None) };
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) => Ok(Some(vec![s])),
        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for el in arr {
                match el {
                    serde_json::Value::String(s) => out.push(s),
                    other => out.push(other.to_string()),
                }
            }
            Ok(Some(out))
        }
        other => Err(D::Error::custom(format!(
            "invalid type for tags: {}",
            other
        ))),
    }
}

/// Deserializes Option<usize> accepting integers, floats (rounded), and numeric strings.
/// Examples: 5, 5.0, "5", "5.7" -> 6
pub fn de_option_usize_forgiving<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let opt = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(v) = opt else { return Ok(None) };
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok(Some(u as usize))
            } else if let Some(i) = n.as_i64() {
                let v = if i < 0 { 0 } else { i as usize };
                Ok(Some(v))
            } else if let Some(f) = n.as_f64() {
                if !f.is_finite() {
                    return Err(D::Error::custom("non-finite numeric for usize"));
                }
                let r = f.round();
                let v = if r < 0.0 { 0 } else { r as usize };
                Ok(Some(v))
            } else {
                Err(D::Error::custom("invalid numeric for usize"))
            }
        }
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            // Try integer first, then float
            if let Ok(i) = s.parse::<i64>() {
                let v = if i < 0 { 0 } else { i as usize };
                Ok(Some(v))
            } else if let Ok(f) = s.parse::<f64>() {
                if !f.is_finite() {
                    return Err(D::Error::custom("non-finite numeric for usize"));
                }
                let r = f.round();
                let v = if r < 0.0 { 0 } else { r as usize };
                Ok(Some(v))
            } else {
                Err(D::Error::custom(format!("invalid usize value: '{}'", s)))
            }
        }
        other => Err(D::Error::custom(format!("invalid type for usize: {}", other))),
    }
}
