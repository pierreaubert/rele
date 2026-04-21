use crate::error::ElispError;
use parking_lot::RwLock;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub type FeatureList = Arc<RwLock<Vec<String>>>;

pub(crate) const MAX_EVAL_DEPTH: usize = 1000;

thread_local! {
    pub(crate) static EVAL_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static MATCH_DATA: RefCell<Vec<Option<(usize, usize)>>> =
        const { RefCell::new(Vec::new()) };
    pub(crate) static MATCH_STRING: RefCell<Option<String>> =
        const { RefCell::new(None) };
    pub(crate) static REGEX_CACHE: RefCell<HashMap<String, regex::Regex>> =
        RefCell::new(HashMap::new());
}

#[inline]
pub fn set_match_data(captures: Vec<Option<(usize, usize)>>, text: Option<String>) {
    MATCH_DATA.with(|d| *d.borrow_mut() = captures);
    MATCH_STRING.with(|s| *s.borrow_mut() = text);
}

#[inline]
pub fn get_match_group(n: usize) -> Option<(usize, usize)> {
    MATCH_DATA.with(|d| d.borrow().get(n).and_then(|x| *x))
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
