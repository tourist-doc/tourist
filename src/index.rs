use crate::config::Config;
use crate::error::{ErrorKind, Result};
use crate::types::path::AbsolutePathBuf;
use failure::ResultExt;
use serde_json;
use std::fs;
use std::path::PathBuf;

pub trait Index: Send + Sync + 'static + Clone {
    fn get(&self, repo_name: &str) -> Result<Option<AbsolutePathBuf>>;
    fn set(&self, repo_name: &str, path: &AbsolutePathBuf) -> Result<()>;
    fn unset(&self, repo_name: &str) -> Result<()>;
    fn all(&self) -> Result<Vec<(String, AbsolutePathBuf)>>;
}

#[derive(Clone)]
pub struct FileIndex;

impl FileIndex {
    fn config_path(&self) -> PathBuf {
        // TODO: Set this with an environment variable
        dirs::home_dir().unwrap().join(".tourist")
    }
}

impl Index for FileIndex {
    fn get(&self, repo_name: &str) -> Result<Option<AbsolutePathBuf>> {
        let config: Config = serde_json::from_str(
            &fs::read_to_string(self.config_path()).context(ErrorKind::FailedToReadIndex)?,
        )
        .context(ErrorKind::FailedToParseIndex)?;
        Ok(config.index.get(repo_name).cloned())
    }

    fn set(&self, repo_name: &str, path: &AbsolutePathBuf) -> Result<()> {
        let mut config: Config = serde_json::from_str(
            &fs::read_to_string(self.config_path()).context(ErrorKind::FailedToReadIndex)?,
        )
        .context(ErrorKind::FailedToParseIndex)?;
        config.index.insert(repo_name.to_owned(), path.clone());
        fs::write(
            self.config_path(),
            serde_json::to_string(&config).context(ErrorKind::FailedToSerializeIndex)?,
        )
        .context(ErrorKind::FailedToWriteIndex)?;
        Ok(())
    }

    fn unset(&self, repo_name: &str) -> Result<()> {
        let mut config: Config = serde_json::from_str(
            &fs::read_to_string(self.config_path()).context(ErrorKind::FailedToReadIndex)?,
        )
        .context(ErrorKind::FailedToParseIndex)?;
        config.index.remove(repo_name);
        fs::write(
            self.config_path(),
            serde_json::to_string(&config).context(ErrorKind::FailedToSerializeIndex)?,
        )
        .context(ErrorKind::FailedToWriteIndex)?;
        Ok(())
    }

    fn all(&self) -> Result<Vec<(String, AbsolutePathBuf)>> {
        let config: Config = serde_json::from_str(
            &fs::read_to_string(self.config_path()).context(ErrorKind::FailedToReadIndex)?,
        )
        .context(ErrorKind::FailedToParseIndex)?;
        Ok(config.index.into_iter().collect())
    }
}
