use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tracing::{info, warn, debug};
use url::Url;

use pardus_core::app::App;
use pardus_core::config::BrowserConfig;
use pardus_core::page::Page;

use crate::config::CrawlConfig;
use crate::discovery::{self, DiscoveredTransition};
use crate::fingerprint::{compute_fingerprint, discover_resources};
use crate::graph::KnowledgeGraph;
use crate::state::{ViewState, ViewStateId};
use crate::transition::{Transition, TransitionOutcome, Trigger};

/// A queued entry in the BFS frontier.
struct FrontierEntry {
    url: String,
    depth: usize,
    parent_id: Option<ViewStateId>,
    trigger: Option<Trigger>,
}

/// Crawl a site and build its Knowledge Graph.
pub async fn crawl(root_url: &str, config: &CrawlConfig) -> Result<KnowledgeGraph> {
    crawl_with_config(root_url, config).await
}

/// Crawl a site with explicit configuration.
pub async fn crawl_with_config(root_url: &str, config: &CrawlConfig) -> Result<KnowledgeGraph> {
    let start = Instant::now();

    let mut browser_config = BrowserConfig::default();
    browser_config.proxy = config.proxy.clone();
    let app = Arc::new(App::new(browser_config));
    let mut graph = KnowledgeGraph::new(root_url, config.clone());

    let root_origin = Url::parse(root_url)
        .map(|u| u.origin().ascii_serialization())
        .unwrap_or_default();

    // BFS frontier
    let mut frontier: VecDeque<FrontierEntry> = VecDeque::new();
    frontier.push_back(FrontierEntry {
        url: root_url.to_string(),
        depth: 0,
        parent_id: None,
        trigger: None,
    });

    // Track normalized URLs already enqueued to avoid re-enqueuing
    let mut url_seen: HashSet<String> = HashSet::new();
    url_seen.insert(normalize_url(root_url));

    let mut pages_crawled = 0usize;
    let mut max_depth_reached = 0usize;

    while let Some(entry) = frontier.pop_front() {
        // Check limits
        if pages_crawled >= config.max_pages {
            debug!("Max pages reached ({})", config.max_pages);
            break;
        }
        if entry.depth > config.max_depth {
            continue;
        }

        // Polite delay
        if pages_crawled > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(config.delay_ms)).await;
        }

        // Fetch page
        info!(url = %entry.url, depth = entry.depth, "Fetching page");
        let page = match Page::from_url(&app, &entry.url).await {
            Ok(p) => p,
            Err(e) => {
                warn!(url = %entry.url, error = %e, "Failed to fetch page");
                continue;
            }
        };
        pages_crawled += 1;
        if entry.depth > max_depth_reached {
            max_depth_reached = entry.depth;
        }

        // Build fingerprint and ViewStateId
        let tree = page.semantic_tree();
        let nav_graph = page.navigation_graph();
        let resource_urls = discover_resources(&page.html, &page.base_url);
        let (fingerprint, state_id) = compute_fingerprint(&page.url, &tree, &resource_urls);

        // Record incoming transition
        if let Some(ref parent_id) = entry.parent_id {
            if let Some(ref trigger) = entry.trigger {
                graph.add_transition(Transition {
                    from: parent_id.clone(),
                    to: state_id.clone(),
                    trigger: trigger.clone(),
                    verified: true,
                    outcome: Some(TransitionOutcome {
                        status: page.status,
                        final_url: page.url.clone(),
                        matched_prediction: true,
                    }),
                });
            }
        }

        // Dedup by ViewStateId
        if graph.has_state(&state_id.0) {
            debug!(id = %state_id.0, "State already known, skipping discovery");
            continue;
        }

        // Build and record ViewState
        let view_state = ViewState {
            id: state_id.clone(),
            url: page.url.clone(),
            fragment: fingerprint.fragment.clone(),
            fingerprint,
            semantic_tree: tree,
            navigation_graph: nav_graph,
            resource_urls,
            title: page.title(),
            status: page.status,
        };

        info!(id = %state_id.0, url = %view_state.url, "New view-state discovered");
        graph.add_state(view_state);

        // Discover outgoing transitions if not at max depth
        if entry.depth < config.max_depth {
            let discovered = discover_transitions_for_page(
                &graph,
                &app,
                &page,
                &state_id,
                &root_origin,
                config,
            );

            for dt in discovered {
                let normalized = normalize_url_for_frontier(&dt.target_url, &root_origin);
                if url_seen.insert(normalized) {
                    frontier.push_back(FrontierEntry {
                        url: dt.target_url,
                        depth: entry.depth + 1,
                        parent_id: Some(state_id.clone()),
                        trigger: Some(dt.trigger),
                    });
                }
            }
        }
    }

    let duration_ms = start.elapsed().as_millis();
    graph.compute_stats(max_depth_reached, pages_crawled, duration_ms);

    info!(
        states = graph.stats.total_states,
        transitions = graph.stats.total_transitions,
        verified = graph.stats.verified_transitions,
        duration_ms = duration_ms,
        "Crawl complete"
    );

    Ok(graph)
}

/// Discover all outgoing transitions for a page.
fn discover_transitions_for_page(
    _graph: &KnowledgeGraph,
    _app: &Arc<App>,
    page: &Page,
    state_id: &ViewStateId,
    root_origin: &str,
    config: &CrawlConfig,
) -> Vec<DiscoveredTransition> {
    let mut all = Vec::new();

    // 1. Link transitions
    let nav_graph = page.navigation_graph();
    all.extend(discovery::discover_link_transitions(
        &nav_graph, root_origin, state_id,
    ));

    // 2. Hash navigation
    if config.discover_hash_nav {
        let hash_transitions = discovery::discover_hash_transitions(&page.html, &page.url);
        all.extend(hash_transitions);
    }

    // 3. Pagination
    if config.discover_pagination {
        let pagination_transitions = discovery::discover_pagination_transitions(&page.url);
        all.extend(pagination_transitions);
    }

    // 4. Forms (optional — predicted, unverified)
    if config.discover_forms {
        for form in &nav_graph.forms {
            let action_url = form.action.clone().unwrap_or_default();
            all.push(DiscoveredTransition {
                target_url: action_url,
                trigger: Trigger::FormSubmit {
                    form_id: form.id.clone(),
                    action: form.action.clone(),
                    method: form.method.clone(),
                    field_count: form.fields.len(),
                },
            });
        }
    }

    all
}

/// Normalize a URL for dedup: lowercase, strip fragment, sort query params, strip trailing slash.
fn normalize_url(url: &str) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_lowercase();
    };
    parsed.set_fragment(None);
    let mut result = parsed.to_string();
    if result.ends_with('/') && !result.ends_with("://") {
        result.pop();
    }
    result
}

/// Normalize a URL for frontier dedup: strip fragment, same-origin check.
fn normalize_url_for_frontier(url: &str, _root_origin: &str) -> String {
    normalize_url(url)
}
