use std::path::{Path, PathBuf};

pub type Component = String;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct RelativePathBuf(Vec<Component>);

impl RelativePathBuf {
    pub fn from_components<I: Iterator<Item = Component>>(i: I) -> Self {
        RelativePathBuf(i.collect())
    }

    pub fn components(&self) -> impl Iterator<Item = &Component> {
        self.0.iter()
    }

    pub fn as_path_buf(&self) -> PathBuf {
        let mut p = PathBuf::new();
        self.0.iter().for_each(|c| p.push(c));
        p
    }

    pub fn as_git_path(&self) -> String {
        self.0.to_vec().join("/")
    }
}

impl From<String> for RelativePathBuf {
    fn from(s: String) -> Self {
        RelativePathBuf::from_components(s.split('/').filter_map(|x| {
            if x.is_empty() {
                None
            } else {
                Some(x.to_owned())
            }
        }))
    }
}

impl From<PathBuf> for RelativePathBuf {
    fn from(p: PathBuf) -> Self {
        RelativePathBuf::from_components(
            p.components()
                .filter_map(|c| c.as_os_str().to_str().map(|x| x.to_owned())),
        )
    }
}

impl From<&Path> for RelativePathBuf {
    fn from(p: &Path) -> Self {
        RelativePathBuf::from_components(
            p.components()
                .filter_map(|c| c.as_os_str().to_str().map(|x| x.to_owned())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::RelativePathBuf;
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;
    use std::path::PathBuf;

    #[test]
    fn from_components_works() {
        let path =
            RelativePathBuf::from_components(vec!["some".to_owned(), "dir".to_owned()].into_iter());
        assert_eq!(path.0.len(), 2);
        assert_eq!(path.0[0], "some");
        assert_eq!(path.0[1], "dir");
    }

    #[quickcheck]
    fn components_consistent_with_path(s: String) -> TestResult {
        let path = PathBuf::from(s.clone());
        if path.is_absolute() {
            return TestResult::discard();
        }
        TestResult::from_bool(
            path.components().count() == RelativePathBuf::from(s).components().count(),
        )
    }

    #[test]
    fn from_str_works() {
        {
            let path: RelativePathBuf = "some/dir".to_owned().into();
            assert_eq!(path.0.len(), 2);
            assert_eq!(path.0[0], "some");
            assert_eq!(path.0[1], "dir");
        }

        {
            let path: RelativePathBuf = "some".to_owned().into();
            assert_eq!(path.0.len(), 1);
            assert_eq!(path.0[0], "some");
        }

        {
            let path: RelativePathBuf = "".to_owned().into();
            assert_eq!(path.0.len(), 0);
        }

        {
            let path: RelativePathBuf = "/some//dir/".to_owned().into();
            assert_eq!(path.0.len(), 2);
            assert_eq!(path.0[0], "some");
            assert_eq!(path.0[1], "dir");
        }
    }
}
