use super::RelativePathBuf;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct AbsolutePathBuf(PathBuf);

impl AbsolutePathBuf {
    pub fn new(p: PathBuf) -> Option<Self> {
        if p.is_absolute() {
            Some(AbsolutePathBuf(p))
        } else {
            None
        }
    }

    pub fn as_absolute_path(&self) -> AbsolutePath<'_> {
        AbsolutePath(&self.0)
    }

    pub fn try_relative(&self, _root: AbsolutePath<'_>) -> Option<RelativePathBuf> {
        unimplemented!();
    }
}

#[derive(PartialEq, Eq)]
pub struct AbsolutePath<'a>(&'a Path);

impl<'a> AbsolutePath<'a> {
    pub fn as_path(&self) -> &Path {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::AbsolutePathBuf;
    use dirs;
    use std::path::Path;

    #[test]
    fn create_abs_path() {
        let abs = dirs::home_dir().unwrap().join("some").join("path");
        let not_abs = Path::new("some").join("path");
        assert!(AbsolutePathBuf::new(abs).is_some());
        assert!(AbsolutePathBuf::new(not_abs).is_none());
    }
}
