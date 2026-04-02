use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use pardus_core::SemanticTree;
use pardus_core::NavigationGraph;

/// Unique fingerprint identifying a distinct page state.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStateId(pub String);

/// Composite fingerprint components that produce a ViewStateId.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fingerprint {
    /// Normalized URL path (no query, no fragment).
    pub url_path: String,
    /// Query params that affect page content (pagination params).
    pub content_query_params: BTreeMap<String, String>,
    /// blake3 hash of the semantic tree's structural skeleton.
    pub tree_hash: String,
    /// blake3 hash of sorted subresource URL set.
    pub resource_set_hash: String,
    /// Hash fragment if present.
    pub fragment: Option<String>,
}

/// A snapshot of a single view-state within the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewState {
    pub id: ViewStateId,
    /// The URL that produced this state.
    pub url: String,
    /// Hash fragment, if present.
    pub fragment: Option<String>,
    /// Fingerprint components.
    pub fingerprint: Fingerprint,
    /// Semantic tree from pardus-core.
    pub semantic_tree: SemanticTree,
    /// Navigation graph from pardus-core.
    pub navigation_graph: NavigationGraph,
    /// The set of subresource URLs loaded by this state.
    pub resource_urls: BTreeSet<String>,
    /// Page title.
    pub title: Option<String>,
    /// HTTP status code.
    pub status: u16,
}
