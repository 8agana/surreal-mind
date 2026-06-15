use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleCliProvider {
    Gemini,
    Antigravity,
}

impl GoogleCliProvider {
    pub fn from_env_or_config(config_value: Option<&str>) -> Result<Self, String> {
        let raw = std::env::var("SM_AGENT_PROVIDER")
            .ok()
            .or_else(|| std::env::var("GOOGLE_CLI_PROVIDER").ok())
            .or_else(|| std::env::var("SURR_GOOGLE_CLI_PROVIDER").ok())
            .or_else(|| config_value.map(str::to_string))
            .unwrap_or_else(|| Self::default_provider().to_string());

        Self::parse(&raw)
    }

    pub fn default_provider() -> Self {
        Self::Antigravity
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "gemini" | "gem" => Ok(Self::Gemini),
            "antigravity" | "agy" => Ok(Self::Antigravity),
            other => Err(format!(
                "unsupported Google CLI provider '{}'; expected 'gemini' or 'antigravity'",
                other
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Gemini => "Gemini CLI",
            Self::Antigravity => "Antigravity CLI",
        }
    }
}

impl fmt::Display for GoogleCliProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gemini => f.write_str("gemini"),
            Self::Antigravity => f.write_str("antigravity"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_aliases() {
        assert_eq!(
            GoogleCliProvider::parse("gemini").unwrap(),
            GoogleCliProvider::Gemini
        );
        assert_eq!(
            GoogleCliProvider::parse("gem").unwrap(),
            GoogleCliProvider::Gemini
        );
        assert_eq!(
            GoogleCliProvider::parse("antigravity").unwrap(),
            GoogleCliProvider::Antigravity
        );
        assert_eq!(
            GoogleCliProvider::parse("agy").unwrap(),
            GoogleCliProvider::Antigravity
        );
    }

    #[test]
    fn rejects_unknown_provider() {
        let err = GoogleCliProvider::parse("bard").unwrap_err();
        assert!(err.contains("unsupported Google CLI provider"));
    }

    #[test]
    fn default_provider_is_antigravity() {
        assert_eq!(
            GoogleCliProvider::default_provider(),
            GoogleCliProvider::Antigravity
        );
    }
}
