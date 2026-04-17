//! Incremental search state (C-s / C-r)

/// Search direction for incremental search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsearchDirection {
    Forward,
    Backward,
}

/// State for incremental search (C-s / C-r).
pub struct IsearchState {
    pub active: bool,
    pub direction: IsearchDirection,
    pub query: String,
    pub start_position: usize,
    pub current_match_start: Option<usize>,
}

impl Default for IsearchState {
    fn default() -> Self {
        Self {
            active: false,
            direction: IsearchDirection::Forward,
            query: String::new(),
            start_position: 0,
            current_match_start: None,
        }
    }
}
