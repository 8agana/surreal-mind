//! Index definitions and validation for SurrealDB tables
//!
//! This module defines the expected indexes for each table and provides
//! utilities for validating index health.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of indexes supported by SurrealDB
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IndexType {
    /// Single-field index
    Single(String),
    /// Multi-field composite index
    Composite(Vec<String>),
}

impl IndexType {
    /// Convert the index into its SurrealDB definition string
    pub fn to_definition(&self) -> String {
        match self {
            IndexType::Single(field) => format!(
                "DEFINE INDEX idx_{} ON TABLE {} FIELDS {}",
                field, "{table}", field
            ),
            IndexType::Composite(fields) => {
                let field_list = fields.join(", ");
                let name = fields.join("_");
                format!(
                    "DEFINE INDEX idx_{} ON TABLE {} FIELDS {}",
                    name, "{table}", field_list
                )
            }
        }
    }
}

/// Expected indexes per table
#[derive(Debug)]
pub struct TableIndexes {
    /// Table name
    pub table: String,
    /// List of required indexes
    pub required: Vec<IndexType>,
    /// Optional performance indexes
    pub optional: Vec<IndexType>,
}

/// Get the expected indexes for all tables
pub fn get_expected_indexes() -> Vec<TableIndexes> {
    vec![
        // Core indexes (essential)
        TableIndexes {
            table: "thoughts".into(),
            required: vec![
                IndexType::Single("created_at".into()),
                IndexType::Single("status".into()),
                IndexType::Single("embedding_model".into()),
            ],
            optional: vec![
                // Performance index for think_search filtering
                IndexType::Single("embedding_dim".into()),
            ],
        },
        TableIndexes {
            table: "kg_entities".into(),
            required: vec![
                IndexType::Single("created_at".into()),
                IndexType::Single("name".into()),
                IndexType::Composite(vec!["name".into(), "data.entity_type".into()]),
            ],
            optional: vec![],
        },
        TableIndexes {
            table: "kg_edges".into(),
            required: vec![
                IndexType::Single("created_at".into()),
                IndexType::Composite(vec!["source".into(), "target".into(), "rel_type".into()]),
            ],
            optional: vec![],
        },
        TableIndexes {
            table: "kg_observations".into(),
            required: vec![
                IndexType::Single("created_at".into()),
                IndexType::Single("name".into()),
                IndexType::Composite(vec!["name".into(), "source_thought_id".into()]),
            ],
            optional: vec![],
        },
        // Extended set (additional indexed tables)
        TableIndexes {
            table: "recalls".into(),
            required: vec![IndexType::Single("created_at".into())],
            optional: vec![],
        },
        TableIndexes {
            table: "kg_entity_candidates".into(),
            required: vec![
                IndexType::Composite(vec!["status".into(), "created_at".into()]),
                IndexType::Single("confidence".into()),
                IndexType::Composite(vec!["name".into(), "entity_type".into(), "status".into()]),
            ],
            optional: vec![],
        },
        TableIndexes {
            table: "kg_edge_candidates".into(),
            required: vec![
                IndexType::Composite(vec!["status".into(), "created_at".into()]),
                IndexType::Single("confidence".into()),
                IndexType::Composite(vec![
                    "source_name".into(),
                    "target_name".into(),
                    "rel_type".into(),
                    "status".into(),
                ]),
            ],
            optional: vec![],
        },
        TableIndexes {
            table: "kg_blocklist".into(),
            required: vec![IndexType::Single("item".into())],
            optional: vec![],
        },
    ]
}

/// Response from INFO FOR TABLE
#[derive(Debug, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub indexes: HashMap<String, IndexInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexInfo {
    pub fields: Vec<String>,
}

/// Result of index health check
#[derive(Debug, Serialize)]
pub struct IndexHealth {
    pub table: String,
    pub expected: Vec<String>,
    pub present: Vec<String>,
    pub missing: Vec<String>,
}
