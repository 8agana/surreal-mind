//! Custom deserializers for flexible parameter parsing in surreal-mind.
//!
//! These deserializers allow the MCP tools to accept parameters in multiple formats,
//! providing a user-friendly interface while maintaining type safety.

use serde::{Deserialize, Deserializer};

/// Deserializes injection_scale parameter with support for numeric values and named presets.
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
            // Validate injection_scale range (0-5)
            if !(0.0..=5.0).contains(&rounded) {
                return Err(D::Error::custom(format!(
                    "injection_scale {} out of range. Must be 0-5",
                    rounded
                )));
            }
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
                            "Invalid injection_scale '{}'. Valid presets: NONE, LIGHT, MEDIUM, DEFAULT, HIGH, MAXIMUM. Or use numeric 0-5.",
                            s
                        ))
                    })?;
                    let rounded = val.round();
                    if !(0.0..=5.0).contains(&rounded) {
                        return Err(D::Error::custom(format!(
                            "injection_scale {} out of range. Must be 0-5 or use presets: NONE, LIGHT, MEDIUM, DEFAULT, HIGH, MAXIMUM",
                            rounded
                        )));
                    }
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
/// - Values outside 0.0-1.0 range after conversion are clamped
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

    // Validate final range
    if !(0.0..=1.0).contains(&val) {
        return Err(D::Error::custom(format!(
            "significance {} out of range. Use 0.0-1.0, 2-10 integer scale, or 'low'/'medium'/'high'",
            val
        )));
    }

    Ok(Some(val))
}
