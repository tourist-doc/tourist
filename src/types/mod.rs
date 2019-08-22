use crate::error::{ErrorKind, Result};
use dirs;
use failure::ResultExt;
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

#[derive(Clone)]
pub struct FileIndex;

pub trait Index: Send + Sync + 'static + Clone {
    fn get(&self, repo_name: &str) -> Result<Option<AbsolutePathBuf>>;
    fn set(&self, repo_name: &str, path: &AbsolutePathBuf) -> Result<()>;
    fn unset(&self, repo_name: &str) -> Result<()>;
    fn all(&self) -> Result<Vec<(String, AbsolutePathBuf)>>;
}

impl FileIndex {
    fn config_path(&self) -> PathBuf {
        dirs::home_dir().unwrap().join(".tourist")
    }
}

impl Index for FileIndex {
    fn get(&self, repo_name: &str) -> Result<Option<AbsolutePathBuf>> {
        let index: HashMap<String, AbsolutePathBuf> = serde_json::from_str(
            &fs::read_to_string(self.config_path()).context(ErrorKind::FailedToReadIndex)?,
        )
        .context(ErrorKind::FailedToParseIndex)?;
        Ok(index.get(repo_name).cloned())
    }

    fn set(&self, repo_name: &str, path: &AbsolutePathBuf) -> Result<()> {
        let mut index: HashMap<String, AbsolutePathBuf> = serde_json::from_str(
            &fs::read_to_string(self.config_path()).context(ErrorKind::FailedToReadIndex)?,
        )
        .context(ErrorKind::FailedToParseIndex)?;
        index.insert(repo_name.to_owned(), path.clone());
        fs::write(
            self.config_path(),
            serde_json::to_string(&index).context(ErrorKind::FailedToSerializeIndex)?,
        )
        .context(ErrorKind::FailedToWriteIndex)?;
        Ok(())
    }

    fn unset(&self, repo_name: &str) -> Result<()> {
        let mut index: HashMap<String, AbsolutePathBuf> = serde_json::from_str(
            &fs::read_to_string(self.config_path()).context(ErrorKind::FailedToReadIndex)?,
        )
        .context(ErrorKind::FailedToParseIndex)?;
        index.remove(repo_name);
        fs::write(
            self.config_path(),
            serde_json::to_string(&index).context(ErrorKind::FailedToSerializeIndex)?,
        )
        .context(ErrorKind::FailedToWriteIndex)?;
        Ok(())
    }

    fn all(&self) -> Result<Vec<(String, AbsolutePathBuf)>> {
        let index: HashMap<String, AbsolutePathBuf> = serde_json::from_str(
            &fs::read_to_string(self.config_path()).context(ErrorKind::FailedToReadIndex)?,
        )
        .context(ErrorKind::FailedToParseIndex)?;
        Ok(index.into_iter().collect())
    }
}
