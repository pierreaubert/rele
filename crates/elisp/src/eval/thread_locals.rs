use super::SyncRefCell as RwLock;
use crate::error::ElispError;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub type FeatureList = Arc<RwLock<Vec<String>>>;

pub(crate) const MAX_EVAL_DEPTH: usize = 50_000;

thread_local! {
    pub(crate) static EVAL_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static REGEX_CACHE: RefCell<HashMap<String, regex::Regex>> =
        RefCell::new(HashMap::new());
}

#[inline]
pub fn inc_eval_depth() -> Result<usize, ElispError> {
    EVAL_DEPTH.with(|d| {
        let new_depth = d.get() + 1;
        if new_depth > MAX_EVAL_DEPTH {
            Err(ElispError::StackOverflow)
        } else {
            d.set(new_depth);
            Ok(new_depth)
        }
    })
}

#[inline]
pub fn dec_eval_depth() {
    EVAL_DEPTH.with(|d| {
        d.set(d.get().saturating_sub(1));
    });
}
