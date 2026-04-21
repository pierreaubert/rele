use crate::object::LispObject;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Macro {
    pub args: LispObject,
    pub body: LispObject,
}

pub type MacroTable = Arc<RwLock<HashMap<String, Macro>>>;
