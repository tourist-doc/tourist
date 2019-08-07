use dirs;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub mod path;

use path::{AbsolutePathBuf, RelativePathBuf};

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

// TODO: Break into trait, mock

pub struct Index;

impl Index {
    fn config_path(&self) -> PathBuf {
        dirs::home_dir().unwrap().join(".tourist")
    }

    pub fn get(&self, repo_name: &str) -> Option<AbsolutePathBuf> {
        let index: HashMap<String, AbsolutePathBuf> =
            serde_json::from_str(&fs::read_to_string(self.config_path()).unwrap()).unwrap();
        index.get(repo_name).cloned()
    }

    pub fn set(&self, repo_name: &str, path: &AbsolutePathBuf) {
        let mut index: HashMap<String, AbsolutePathBuf> =
            serde_json::from_str(&fs::read_to_string(self.config_path()).unwrap()).unwrap();
        index.insert(repo_name.to_owned(), path.clone());
        fs::write(self.config_path(), serde_json::to_string(&index).unwrap()).unwrap();
    }

    pub fn unset(&self, repo_name: &str) {
        let mut index: HashMap<String, AbsolutePathBuf> =
            serde_json::from_str(&fs::read_to_string(self.config_path()).unwrap()).unwrap();
        index.remove(repo_name);
        fs::write(self.config_path(), serde_json::to_string(&index).unwrap()).unwrap();
    }

    pub fn all(&self) -> Vec<(String, AbsolutePathBuf)> {
        let index: HashMap<String, AbsolutePathBuf> =
            serde_json::from_str(&fs::read_to_string(self.config_path()).unwrap()).unwrap();
        index.into_iter().collect()
    }
}
