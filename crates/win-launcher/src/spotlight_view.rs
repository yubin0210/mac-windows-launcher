//! spotlight_view
//! State machine and keyboard navigation for the Spotlight search interface.
//!
//! Manages:
//! - Search input buffer and incremental updates (< 1ms search turnaround).
//! - Result ranking and selection state.
//! - Arrow navigation with toroidal wrap-around.
//! - Calculation detection and inspection.

use crate::indexer::AppIndexer;
use mac_physics_core::spotlight::{SpotlightCategory, SpotlightItem};

/// State machine managing the Spotlight search session.
#[derive(Debug, Clone)]
pub struct SpotlightView {
    indexer: AppIndexer,
    query: String,
    results: Vec<SpotlightItem>,
    selected_index: usize,
    limit: usize,
}

impl SpotlightView {
    /// Creates a new `SpotlightView` backed by the given indexer.
    pub fn new(indexer: AppIndexer) -> Self {
        Self {
            indexer,
            query: String::new(),
            results: Vec::new(),
            selected_index: 0,
            limit: 5,
        }
    }

    /// Sets the maximum number of results to display.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        self.update_results();
    }

    /// Appends typed text to the query buffer and refreshes results.
    pub fn input_text(&mut self, text: &str) {
        self.query.push_str(text);
        self.selected_index = 0;
        self.update_results();
    }

    /// Sets the exact query string.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.selected_index = 0;
        self.update_results();
    }

    /// Deletes the last character in the query buffer.
    pub fn delete_char(&mut self) {
        self.query.pop();
        self.selected_index = 0;
        self.update_results();
    }

    /// Clears the query buffer and results.
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
        self.selected_index = 0;
    }

    /// Navigates the selected row with wrap-around (-1 for Up, +1 for Down).
    pub fn navigate(&mut self, delta: i32) {
        if self.results.is_empty() {
            self.selected_index = 0;
            return;
        }

        let count = self.results.len() as i32;
        let current = self.selected_index as i32;
        let next = (current + delta).rem_euclid(count);
        self.selected_index = next as usize;
    }

    /// Returns the current query text.
    pub fn get_query(&self) -> &str {
        &self.query
    }

    /// Returns the active search results.
    pub fn get_results(&self) -> &[SpotlightItem] {
        &self.results
    }

    /// Index of the currently highlighted result row.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Explicitly sets the selected index if within bounds.
    pub fn set_selected_index(&mut self, idx: usize) {
        if idx < self.results.len() {
            self.selected_index = idx;
        }
    }

    /// Returns a reference to the currently selected result item.
    pub fn selected_item(&self) -> Option<&SpotlightItem> {
        self.results.get(self.selected_index)
    }

    /// Number of active results.
    pub fn results_count(&self) -> usize {
        self.results.len()
    }

    /// True if there are no search results.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// True if the top result is an instant calculation result.
    pub fn has_calculation(&self) -> bool {
        self.results
            .first()
            .map_or(false, |it| it.category == SpotlightCategory::Calculation)
    }

    /// Reference to the underlying indexer.
    pub fn indexer(&self) -> &AppIndexer {
        &self.indexer
    }

    /// Mutable reference to the underlying indexer.
    pub fn indexer_mut(&mut self) -> &mut AppIndexer {
        &mut self.indexer
    }

    /// Refreshes search results based on current query.
    fn update_results(&mut self) {
        let q = self.query.trim();
        if q.is_empty() {
            self.results.clear();
            self.selected_index = 0;
            return;
        }

        self.results = self.indexer.search(q, self.limit);

        if self.selected_index >= self.results.len() {
            self.selected_index = 0;
        }
    }
}
