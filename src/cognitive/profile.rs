//! Submode profiles and tunings with defaults.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submode {
    Sarcastic,
    Philosophical,
    Empathetic,
    ProblemSolving,
}

#[allow(clippy::should_implement_trait)]
impl Submode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "sarcastic" => Submode::Sarcastic,
            "philosophical" => Submode::Philosophical,
            "empathetic" => Submode::Empathetic,
            _ => Submode::ProblemSolving,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InjectionTuning {
    pub threshold_delta: f32,
    pub favor: &'static str, // memory flavor key
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OrbitalTuning {
    pub age_w: f32,
    pub access_w: f32,
    pub significance_w: f32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RelevanceTuning {
    pub sim_w: f32,
    pub orbital_w: f32,
    pub flavor_bonus: f32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SubmodeProfile {
    pub weights: HashMap<&'static str, u8>,
    pub injection: InjectionTuning,
    pub orbital: OrbitalTuning,
    pub relevance: RelevanceTuning,
}

pub fn profile_for(submode: Submode) -> SubmodeProfile {
    match submode {
        Submode::Sarcastic => SubmodeProfile {
            weights: HashMap::from([
                ("OODA", 60),
                ("Socratic", 30),
                ("Lateral", 10),
                ("FirstPrinciples", 0),
                ("RootCause", 0),
                ("SystemsThinking", 0),
                ("Dialectical", 0),
            ]),
            injection: InjectionTuning {
                threshold_delta: -0.05,
                favor: "contrarian",
            },
            orbital: OrbitalTuning {
                age_w: 0.35,
                access_w: 0.35,
                significance_w: 0.30,
            },
            relevance: RelevanceTuning {
                sim_w: 0.60,
                orbital_w: 0.40,
                flavor_bonus: 0.03,
            },
        },
        Submode::Philosophical => SubmodeProfile {
            weights: HashMap::from([
                ("FirstPrinciples", 40),
                ("Socratic", 40),
                ("Lateral", 20),
                ("OODA", 0),
                ("RootCause", 0),
                ("SystemsThinking", 0),
                ("Dialectical", 0),
            ]),
            injection: InjectionTuning {
                threshold_delta: 0.0,
                favor: "abstract",
            },
            orbital: OrbitalTuning {
                age_w: 0.30,
                access_w: 0.25,
                significance_w: 0.45,
            },
            relevance: RelevanceTuning {
                sim_w: 0.60,
                orbital_w: 0.40,
                flavor_bonus: 0.02,
            },
        },
        Submode::Empathetic => SubmodeProfile {
            weights: HashMap::from([
                ("RootCause", 50),
                ("Socratic", 30),
                ("FirstPrinciples", 20),
                ("OODA", 0),
                ("Lateral", 0),
                ("SystemsThinking", 0),
                ("Dialectical", 0),
            ]),
            injection: InjectionTuning {
                threshold_delta: 0.02,
                favor: "emotional",
            },
            orbital: OrbitalTuning {
                age_w: 0.30,
                access_w: 0.20,
                significance_w: 0.50,
            },
            relevance: RelevanceTuning {
                sim_w: 0.60,
                orbital_w: 0.40,
                flavor_bonus: 0.03,
            },
        },
        Submode::ProblemSolving => SubmodeProfile {
            weights: HashMap::from([
                ("OODA", 40),
                ("RootCause", 30),
                ("FirstPrinciples", 30),
                ("Socratic", 0),
                ("Lateral", 0),
                ("SystemsThinking", 0),
                ("Dialectical", 0),
            ]),
            injection: InjectionTuning {
                threshold_delta: 0.0,
                favor: "solution",
            },
            orbital: OrbitalTuning {
                age_w: 0.40,
                access_w: 0.30,
                significance_w: 0.30,
            },
            relevance: RelevanceTuning {
                sim_w: 0.60,
                orbital_w: 0.40,
                flavor_bonus: 0.02,
            },
        },
    }
}
