use std::collections::HashMap;

pub mod path;

use path::{AbsolutePathBuf, RelativePathBuf};

pub struct StopReference {
    pub tour_id: String,
    pub stop_id: String,
}

pub struct Stop {
    pub id: String,
    pub title: String,
    pub description: String,
    pub path: RelativePathBuf,
    pub repository: String,
    pub line: usize,
    pub children: Vec<StopReference>,
}

pub struct Tour {
    pub protocol_version: String,
    pub id: String,
    pub title: String,
    pub description: String,
    pub stops: Vec<Stop>,
    pub repositories: HashMap<String, String>,
    pub generator: usize,
}

pub type Index = HashMap<String, AbsolutePathBuf>;
