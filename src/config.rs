use crate::error::{ErrorKind, Result};
use crate::serialize;
use crate::types::path::AbsolutePathBuf;
use crate::types::Tour;
use failure::ResultExt;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

pub fn config_path() -> PathBuf {
    // TODO: Set this with an environment variable
    dirs::home_dir().unwrap().join(".tourist")
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub index: HashMap<String, AbsolutePathBuf>,
    pub dirs: Vec<AbsolutePathBuf>,
}

pub fn get_default_tours() -> Result<Vec<Tour>> {
    let config: Config = serde_json::from_str(
        &fs::read_to_string(config_path()).context(ErrorKind::FailedToReadIndex)?,
    )
    .context(ErrorKind::FailedToParseIndex)?;
    let mut tours = vec![];
    // TODO: Make recursive
    for dir in config.dirs {
        let entries = dir
            .as_path_buf()
            .read_dir()
            .context(ErrorKind::FailedToParseIndex)?;
        for entry in entries {
            let path = entry.context(ErrorKind::FailedToReadIndex)?.path();
            if path.extension().and_then(OsStr::to_str) == Some(".tour") {
                let tour = serialize::parse_tour(
                    &fs::read_to_string(path).context(ErrorKind::FailedToReadTour)?,
                )
                .context(ErrorKind::FailedToParseTour)?;
                tours.push(tour);
            }
        }
    }
    Ok(tours)
}
