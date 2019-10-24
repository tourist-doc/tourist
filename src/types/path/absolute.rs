use super::RelativePathBuf;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Hash)]
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

    pub fn try_relative(&self, root: AbsolutePath<'_>) -> Option<RelativePathBuf> {
        let deep = self.as_absolute_path();
        let mut deep_components = deep.components().peekable();
        let mut root_components = root.components().peekable();

        // Go through path, and keep going as long as components match
        while let (Some(d), Some(r)) = (deep_components.peek(), root_components.peek()) {
            // If components ever don't match, relativization fails
            if d != r {
                return None;
            }
            deep_components.next();
            root_components.next();
        }

        if root_components.next().is_some() {
            // If there are still root components left, that is also an error case
            None
        } else {
            // Otherwise the remainder of the deep path is the relative path
            Some(RelativePathBuf::from_components(
                deep_components.map(|s| s.to_owned()),
            ))
        }
    }

    pub fn as_path_buf(&self) -> &PathBuf {
        &self.0
    }

    pub fn join_rel(&self, rel_path: &RelativePathBuf) -> AbsolutePathBuf {
        let mut path = self.0.clone();
        for comp in rel_path.components() {
            path.push(comp);
        }
        AbsolutePathBuf(path)
    }
}

#[derive(PartialEq, Eq, Copy, Clone, Hash)]
pub struct AbsolutePath<'a>(&'a Path);

impl<'a> AbsolutePath<'a> {
    pub fn as_path(&self) -> &Path {
        self.0
    }

    fn components(&self) -> impl Iterator<Item = &str> {
        self.0.components().map(|c| {
            c.as_os_str().to_str().unwrap_or_else(|| {
                panic!(format!(
                    "path component {:?} invalid unicode",
                    c.as_os_str(),
                ));
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AbsolutePathBuf, RelativePathBuf};
    use dirs;
    use std::path::Path;

    #[test]
    fn create_abs_path() {
        let abs = dirs::home_dir()
            .expect("no home dir")
            .join("some")
            .join("path");
        let not_abs = Path::new("some").join("path");
        assert!(AbsolutePathBuf::new(abs).is_some());
        assert!(AbsolutePathBuf::new(not_abs).is_none());
    }

    #[test]
    fn simple_try_relative() {
        // Relativize $HOME/some/path/and/more from $HOME/some/path, expect and/more
        let root = AbsolutePathBuf::new(
            dirs::home_dir()
                .expect("no home dir")
                .join("some")
                .join("path"),
        )
        .expect("path not absolute");
        let path = AbsolutePathBuf::new(root.as_path_buf().clone().join("and").join("more"))
            .expect("path not absolute");
        assert_eq!(
            Some(RelativePathBuf::from_components(
                vec!["and".to_owned(), "more".to_owned()].into_iter()
            )),
            path.try_relative(root.as_absolute_path()),
        );
    }

    #[test]
    fn unrelated_try_relative() {
        // Relativize $DOWNLOADS/other/thing from $HOME/some/path, expect <none>
        let root = AbsolutePathBuf::new(
            dirs::home_dir()
                .expect("no home dir")
                .join("some")
                .join("path"),
        )
        .expect("path not absolute");
        let path = AbsolutePathBuf::new(
            dirs::download_dir()
                .expect("no download dir")
                .join("other")
                .join("thing"),
        )
        .expect("path not absolute");
        assert!(path.try_relative(root.as_absolute_path()).is_none());
    }

    #[test]
    fn same_root_unrelated_try_relative() {
        // Relativize $HOME/some/path/foo from $HOME/some/path/bar, expect <none>
        let root = AbsolutePathBuf::new(
            dirs::home_dir()
                .expect("no home dir")
                .join("some")
                .join("path")
                .join("foo"),
        )
        .expect("path not absolute");
        let path = AbsolutePathBuf::new(
            dirs::home_dir()
                .expect("no home dir")
                .join("some")
                .join("path")
                .join("bar"),
        )
        .expect("path not absolute");
        assert!(path.try_relative(root.as_absolute_path()).is_none());
    }

    #[test]
    fn empty_try_relative() {
        // Relativize $HOME/some/path from $HOME/some/path, expect <empty>
        let root = AbsolutePathBuf::new(
            dirs::home_dir()
                .expect("no home dir")
                .join("some")
                .join("path"),
        )
        .expect("path not absolute");
        let path = AbsolutePathBuf::new(root.as_path_buf().clone()).expect("path not absolute");
        assert_eq!(
            Some(RelativePathBuf::from_components(vec![].into_iter())),
            path.try_relative(root.as_absolute_path()),
        );
    }

    #[test]
    fn file_try_relative() {
        // Relativize $HOME/some/path/foo.txt from $HOME/some/path, expect foo.txt
        let root = AbsolutePathBuf::new(
            dirs::home_dir()
                .expect("no home dir")
                .join("some")
                .join("path"),
        )
        .expect("path not absolute");
        let path = AbsolutePathBuf::new(root.as_path_buf().clone().join("foo.txt"))
            .expect("path not absolute");
        assert_eq!(
            Some(RelativePathBuf::from_components(
                vec!["foo.txt".to_owned()].into_iter()
            )),
            path.try_relative(root.as_absolute_path()),
        );
    }
}
