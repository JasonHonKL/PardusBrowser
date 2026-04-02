use anyhow::Result;

use crate::graph::KnowledgeGraph;

/// Serialize the knowledge graph to pretty-printed JSON.
pub fn serialize_kg(kg: &KnowledgeGraph) -> Result<String> {
    Ok(serde_json::to_string_pretty(kg)?)
}
