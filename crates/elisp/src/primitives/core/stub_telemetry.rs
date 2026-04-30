use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::obarray::SymbolId;

thread_local! {
    static STUB_HITS: RefCell<BTreeMap<String, usize>> = const {
        RefCell::new(BTreeMap::new())
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StubHit {
    pub name: String,
    pub count: usize,
}

pub(crate) fn clear_stub_hits() {
    STUB_HITS.with(|hits| hits.borrow_mut().clear());
}

pub(crate) fn record_stub_hit(name: &str) {
    STUB_HITS.with(|hits| {
        *hits.borrow_mut().entry(name.to_string()).or_default() += 1;
    });
}

pub(crate) fn record_primitive_alias_hit(primitive_name: &str, caller_sym: Option<SymbolId>) {
    let Some(sym) = caller_sym else {
        return;
    };
    let caller_name = crate::obarray::symbol_name(sym);
    if caller_name == primitive_name {
        return;
    }
    record_stub_hit(&format!("{caller_name}->{primitive_name}"));
}

pub(crate) fn take_stub_hits() -> Vec<StubHit> {
    STUB_HITS.with(|hits| {
        let mut hits = hits.borrow_mut();
        std::mem::take(&mut *hits)
            .into_iter()
            .map(|(name, count)| StubHit { name, count })
            .collect()
    })
}

pub(crate) fn encode_stub_hits(hits: &[StubHit]) -> String {
    hits.iter()
        .map(|hit| format!("{}={}", hit.name, hit.count))
        .collect::<Vec<_>>()
        .join(";")
}
