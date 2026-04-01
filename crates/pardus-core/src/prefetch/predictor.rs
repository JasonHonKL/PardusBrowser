//! Navigation prediction using Markov chains

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::trace;

/// Simple Markov chain model for navigation prediction
pub struct NavigationPredictor {
    /// Transition counts: (from, to) -> count
    transitions: DashMap<(String, String), usize>,
    /// Page visit counts
    visit_counts: DashMap<String, usize>,
    /// Current session path
    session_path: parking_lot::Mutex<Vec<String>>,
}

impl NavigationPredictor {
    pub fn new() -> Self {
        Self {
            transitions: DashMap::new(),
            visit_counts: DashMap::new(),
            session_path: parking_lot::Mutex::new(Vec::new()),
        }
    }

    /// Record a navigation transition
    pub fn record_transition(
        &self,
        from: &str,
        to: &str,
    ) {
        let key = (from.to_string(), to.to_string());
        
        // Increment transition count
        *self.transitions.entry(key).or_insert(0) += 1;
        
        // Increment visit count
        *self.visit_counts.entry(to.to_string()).or_insert(0) += 1;
        
        // Update session path
        self.session_path.lock().push(from.to_string());
        
        trace!("recorded transition: {} -> {}", from, to);
    }

    /// Predict next URLs based on current URL
    pub fn predict_next(
        &self,
        current_url: &str,
        max_predictions: usize,
    ) -> Vec<String> {
        let mut candidates: Vec<(String, f64)> = Vec::new();
        
        // Get all transitions from current URL
        for entry in self.transitions.iter() {
            let ((from, to), count) = entry.pair();
            if from == current_url {
                let probability = self.compute_probability(current_url, to, *count);
                candidates.push((to.clone(), probability));
            }
        }
        
        // Sort by probability (descending)
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // Return top N
        candidates.into_iter()
            .take(max_predictions)
            .map(|(url, _)| url)
            .collect()
    }

    /// Compute transition probability
    fn compute_probability(
        &self,
        from: &str,
        to: &str,
        count: usize,
    ) -> f64 {
        // Total transitions from this page
        let total: usize = self.transitions.iter()
            .filter(|e| e.key().0 == from)
            .map(|e| *e.value())
            .sum();
        
        if total == 0 {
            return 0.0;
        }
        
        // P(to | from) = count(from -> to) / count(from -> *)
        let base_prob = count as f64 / total as f64;
        
        // Boost based on overall popularity
        let visit_count = self.visit_counts.get(to).map(|v| *v).unwrap_or(0);
        let popularity_boost = (visit_count as f64).ln_1p() / 10.0;
        
        (base_prob + popularity_boost).min(1.0)
    }

    /// Predict based on sequence of recent pages
    pub fn predict_from_sequence(
        &self,
        sequence: &[PageSequence],
        max_predictions: usize,
    ) -> Vec<String> {
        if sequence.is_empty() {
            return Vec::new();
        }
        
        // Use last page as primary predictor
        let last = sequence.last().unwrap().url.as_str();
        self.predict_next(last, max_predictions)
    }

    /// Get predictor statistics
    pub fn stats(&self) -> PredictorStats {
        PredictorStats {
            transitions: self.transitions.len(),
            unique_pages: self.visit_counts.len(),
        }
    }
}

impl Default for NavigationPredictor {
    fn default() -> Self {
        Self::new()
    }
}

/// Page sequence entry
#[derive(Debug, Clone)]
pub struct PageSequence {
    pub url: String,
    pub timestamp: std::time::Instant,
    pub dwell_time_ms: u64,
}

/// Markov chain model for navigation
pub struct NavigationModel {
    /// Order of Markov chain (1 = first-order, 2 = second-order, etc.)
    order: usize,
    /// State transitions
    transitions: DashMap<Vec<String>, HashMap<String, usize>>,
}

impl NavigationModel {
    pub fn new(order: usize) -> Self {
        Self {
            order,
            transitions: DashMap::new(),
        }
    }

    /// Train on a sequence of page visits
    pub fn train(&self,
        sequence: &[PageSequence],
    ) {
        if sequence.len() <= self.order {
            return;
        }
        
        for window in sequence.windows(self.order + 1) {
            let state: Vec<String> = window[..self.order]
                .iter()
                .map(|p| p.url.clone())
                .collect();
            let next = window[self.order].url.clone();
            
            self.transitions
                .entry(state)
                .or_insert_with(HashMap::new)
                .entry(next)
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
    }

    /// Predict next page from current state
    pub fn predict(&self,
        current_state: &[PageSequence],
    ) -> Option<String> {
        let state: Vec<String> = current_state.iter()
            .rev()
            .take(self.order)
            .rev()
            .map(|p| p.url.clone())
            .collect();
        
        self.transitions.get(&state).and_then(|transitions| {
            // Return most likely next state
            transitions.iter()
                .max_by_key(|(_, count)| *count)
                .map(|(url, _)| url.clone())
        })
    }
}

/// Predictor statistics
#[derive(Debug, Clone)]
pub struct PredictorStats {
    pub transitions: usize,
    pub unique_pages: usize,
}
