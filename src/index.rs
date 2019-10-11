use crate::config::{config, write_config, Config};
use crate::error::Result;
use crate::types::path::AbsolutePathBuf;

pub trait Index {
    fn get(&self, repo_name: &str) -> Result<Option<AbsolutePathBuf>>;
    fn set(&self, repo_name: &str, path: &AbsolutePathBuf) -> Result<()>;
    fn unset(&self, repo_name: &str) -> Result<()>;
    fn all(&self) -> Result<Vec<(String, AbsolutePathBuf)>>;
}

#[derive(Clone)]
pub struct FileIndex;

impl Index for FileIndex {
    fn get(&self, repo_name: &str) -> Result<Option<AbsolutePathBuf>> {
        let config: Config = config();
        Ok(config.index.get(repo_name).cloned())
    }

    fn set(&self, repo_name: &str, path: &AbsolutePathBuf) -> Result<()> {
        let mut config: Config = config();
        config.index.insert(repo_name.to_owned(), path.clone());
        write_config(config)?;
        Ok(())
    }

    fn unset(&self, repo_name: &str) -> Result<()> {
        let mut config: Config = config();
        config.index.remove(repo_name);
        write_config(config)?;
        Ok(())
    }

    fn all(&self) -> Result<Vec<(String, AbsolutePathBuf)>> {
        let config: Config = config();
        Ok(config.index.into_iter().collect())
    }
}
