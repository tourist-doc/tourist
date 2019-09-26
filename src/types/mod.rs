use std::collections::HashMap;

#[cfg(test)]
pub mod arbitrary;
pub mod path;

use path::RelativePathBuf;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct StopReference {
    pub tour_id: String,
    pub stop_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Stop {
    pub id: String,
    pub title: String,
    pub description: String,
    pub path: RelativePathBuf,
    pub repository: String,
    pub line: usize,
    pub children: Vec<StopReference>,
    /// If `None`, the stop is not broken. If `Some(s)`, `s` should hold a short message about what
    /// went wrong.
    pub broken: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Tour {
    pub protocol_version: String,
    pub id: String,
    pub title: String,
    pub description: String,
    pub stops: Vec<Stop>,
    pub repositories: HashMap<String, String>,
    pub generator: usize,
}
