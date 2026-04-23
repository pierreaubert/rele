use crate::object::LispObject;
use super::SyncRefCell;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Macro {
    pub args: LispObject,
    pub body: LispObject,
}

pub type MacroTable = Arc<SyncRefCell<HashMap<String, Macro>>>;
