use crate::types::path::AbsolutePathBuf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub index: HashMap<String, AbsolutePathBuf>,
    pub path: Vec<AbsolutePathBuf>,
}
