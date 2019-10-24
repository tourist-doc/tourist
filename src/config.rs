use crate::error::{ErrorKind, Result};
use crate::serialize;
use crate::types::path::AbsolutePathBuf;
use crate::types::Tour;
use dotenv;
use failure::ResultExt;
use serde::{Deserialize, Serialize};
use serde_json;
use slog_scope::error;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    if let Ok(p) = dotenv::var("TOURIST_CONFIG") {
        PathBuf::from(p)
    } else {
        dirs::home_dir()
            .expect("Operating system does not have a home directory.")
            .join(".tourist")
    }
}

pub fn config() -> Config {
    let config = fs::read_to_string(config_path())
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok());
    if let Some(c) = config {
        c
    } else {
        error!(
            "failed to parse config file at '{}', running with default config",
            config_path().display()
        );
        Config {
            index: HashMap::new(),
            dirs: vec![],
        }
    }
}

pub fn write_config(config: Config) -> Result<()> {
    fs::write(
        config_path(),
        serde_json::to_string(&config).context(ErrorKind::FailedToSerializeIndex)?,
    )
    .context(ErrorKind::FailedToWriteIndex)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub index: HashMap<String, AbsolutePathBuf>,
    pub dirs: Vec<AbsolutePathBuf>,
}

pub fn get_default_tours() -> Result<Vec<(Tour, PathBuf)>> {
    let config: Config = config();
    collect_tours(config.dirs.clone())
}

fn collect_tours(mut stack: Vec<AbsolutePathBuf>) -> Result<Vec<(Tour, PathBuf)>> {
    let mut tours = vec![];
    while let Some(dir) = stack.pop() {
        let entries = dir
            .as_path_buf()
            .read_dir()
            .context(ErrorKind::FailedToParseIndex)?;
        for entry in entries {
            let path = entry.context(ErrorKind::FailedToReadIndex)?.path();
            if path.is_dir() {
                stack.push(
                    AbsolutePathBuf::new(path)
                        .expect("read_dir should always return entries with absolute paths"),
                );
            } else if path.extension().and_then(OsStr::to_str) == Some("tour") {
                let tour = serialize::parse_tour(
                    &fs::read_to_string(&path).context(ErrorKind::FailedToReadTour)?,
                )
                .context(ErrorKind::FailedToParseTour)?;
                tours.push((tour, path));
            }
        }
    }
    Ok(tours)
}

#[cfg(test)]
mod tests {
    use super::collect_tours;
    use crate::types::path::AbsolutePathBuf;
    use crate::types::Tour;
    use std::fs;
    use std::path::Path;
    use tempdir::TempDir;

    fn write_basic_tour(path: &Path) {
        // This is super hacky, but it doesn't need to be robust. This is just to make sure I can
        // create tour files arbitrarily far down a potentially non-existant directory tree.
        if !path.parent().expect("no parent").exists() {
            fs::create_dir_all(path.parent().expect("no parent")).expect("could not create dir");
        }
        fs::write(
            path,
            crate::serialize::serialize_tour(Tour {
                id: "TOURID".to_owned(),
                title: "My first tour".to_owned(),
                description: "".to_owned(),
                stops: vec![],
                protocol_version: "1.0".to_owned(),
                repositories: vec![].into_iter().collect(),
            })
            .expect("failed to serialize"),
        )
        .expect("failed to write");
    }

    #[test]
    fn collect_tours_works() {
        let temp_dir = TempDir::new("collect_tours_works").expect("TempDir failed");
        write_basic_tour(&temp_dir.path().join("example.tour"));
        let tours = collect_tours(vec![
            AbsolutePathBuf::new(temp_dir.path().to_path_buf()).expect("not absolute")
        ])
        .expect("collect failed");
        assert_eq!(tours[0].0.title, "My first tour");
        assert_eq!(tours[0].0.stops.len(), 0);

        assert_eq!(tours[0].1, temp_dir.path().join("example.tour"));
    }

    #[test]
    fn collect_tours_recursively() {
        let temp_dir = TempDir::new("collect_tours_recursive").expect("TempDir failed");
        write_basic_tour(&temp_dir.path().join("example1.tour"));
        write_basic_tour(&temp_dir.path().join("between").join("example2.tour"));
        write_basic_tour(
            &temp_dir
                .path()
                .join("two")
                .join("down")
                .join("example3.tour"),
        );
        write_basic_tour(&temp_dir.path().join("between").join("example4.tour"));
        let tours = collect_tours(vec![
            AbsolutePathBuf::new(temp_dir.into_path()).expect("not absolute")
        ])
        .expect("collect failed");
        assert_eq!(tours.len(), 4);
    }
}
