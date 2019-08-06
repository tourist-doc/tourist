use crate::types::path::RelativePathBuf;
use std::collections::{BTreeSet, HashMap};
use std::convert::TryInto;

pub struct DiffFileEvent {
    pub from: RelativePathBuf,
    pub to: Option<RelativePathBuf>,
}

pub struct DiffLineEvent {
    pub key: RelativePathBuf,
    pub from: Option<u32>,
    pub to: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Changes(HashMap<RelativePathBuf, FileChanges>);

impl Changes {
    pub fn new() -> Self {
        Changes(HashMap::new())
    }

    pub fn for_file(&self, path: &RelativePathBuf) -> Option<&FileChanges> {
        self.0.get(path)
    }

    pub fn process_file(&mut self, e: DiffFileEvent) {
        match e.to {
            None => self.0.insert(e.from, FileChanges::Deleted),
            Some(to_path) => {
                if to_path == e.from {
                    self.0.insert(e.from, FileChanges::new_changed())
                } else {
                    self.0.insert(e.from, FileChanges::new_renamed(to_path))
                }
            }
        };
    }

    pub fn process_line(&mut self, e: DiffLineEvent) {
        let from = e.from.map(|v| v.try_into().unwrap());
        let to = e.to.map(|v| v.try_into().unwrap());
        self.0.entry(e.key).and_modify(|m| {
            match (from, to) {
                (Some(from), Some(to)) => m.line_moved(from, to),
                (None, Some(to)) => m.line_added(to),
                (Some(from), None) => m.line_deleted(from),
                _ => (),
            };
        });
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FileChanges {
    Deleted,
    Renamed {
        new_name: RelativePathBuf,
        changes: HashMap<usize, usize>,
        additions: BTreeSet<usize>,
        deletions: BTreeSet<usize>,
    },
    Changed {
        changes: HashMap<usize, usize>,
        additions: BTreeSet<usize>,
        deletions: BTreeSet<usize>,
    },
}

impl FileChanges {
    fn new_changed() -> Self {
        FileChanges::Changed {
            changes: HashMap::new(),
            additions: BTreeSet::new(),
            deletions: BTreeSet::new(),
        }
    }

    fn new_renamed(path: RelativePathBuf) -> Self {
        FileChanges::Renamed {
            new_name: path,
            changes: HashMap::new(),
            additions: BTreeSet::new(),
            deletions: BTreeSet::new(),
        }
    }

    fn line_moved(&mut self, from: usize, to: usize) {
        match self {
            FileChanges::Renamed { changes, .. } => {
                changes.insert(from, to);
            }
            FileChanges::Changed { changes, .. } => {
                changes.insert(from, to);
            }
            FileChanges::Deleted => {}
        }
    }

    fn line_added(&mut self, to: usize) {
        match self {
            FileChanges::Renamed { additions, .. } => {
                additions.insert(to);
            }
            FileChanges::Changed { additions, .. } => {
                additions.insert(to);
            }
            FileChanges::Deleted => {}
        }
    }

    fn line_deleted(&mut self, from: usize) {
        match self {
            FileChanges::Renamed { deletions, .. } => {
                deletions.insert(from);
            }
            FileChanges::Changed { deletions, .. } => {
                deletions.insert(from);
            }
            FileChanges::Deleted => {}
        }
    }

    pub fn adjust_line(&self, line: usize) -> Option<usize> {
        let (changes, additions, deletions) = match self {
            FileChanges::Deleted => return None,
            FileChanges::Changed {
                changes,
                additions,
                deletions,
            } => (changes, additions, deletions),
            FileChanges::Renamed {
                changes,
                additions,
                deletions,
                ..
            } => (changes, additions, deletions),
        };
        if deletions.contains(&line) {
            return None;
        }
        if let Some(&dest) = changes.get(&line) {
            return Some(dest);
        }
        let mut dest = line;
        // Iterate over additions in ascending order
        for added_line in additions.iter() {
            if *added_line < dest {
                dest += 1;
            } else {
                break;
            }
        }
        Some(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::{Changes, DiffFileEvent, DiffLineEvent, FileChanges};
    use crate::types::path::RelativePathBuf;

    #[test]
    fn file_events_work() {
        let file_events = vec![
            DiffFileEvent {
                from: RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
                to: Some(RelativePathBuf::from("foo/bar/baz.txt".to_owned())),
            },
            DiffFileEvent {
                from: RelativePathBuf::from("foo/qux.txt".to_owned()),
                to: None,
            },
            DiffFileEvent {
                from: RelativePathBuf::from("hello/world.md".to_owned()),
                to: Some(RelativePathBuf::from("hello/world/doc.md".to_owned())),
            },
        ];

        let mut expected = Changes::new();
        expected.0.insert(
            RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
            FileChanges::new_changed(),
        );
        expected.0.insert(
            RelativePathBuf::from("foo/qux.txt".to_owned()),
            FileChanges::Deleted,
        );
        expected.0.insert(
            RelativePathBuf::from("hello/world.md".to_owned()),
            FileChanges::new_renamed(RelativePathBuf::from("hello/world/doc.md".to_owned())),
        );

        let mut actual = Changes::new();
        file_events.into_iter().for_each(|e| {
            actual.process_file(e);
        });

        assert_eq!(expected, actual);
    }

    #[test]
    fn line_events_work() {
        let line_events = vec![
            DiffLineEvent {
                key: RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
                from: Some(12),
                to: Some(12),
            },
            DiffLineEvent {
                key: RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
                from: Some(2),
                to: Some(3),
            },
            DiffLineEvent {
                key: RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
                from: None,
                to: Some(100),
            },
            DiffLineEvent {
                key: RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
                from: Some(200),
                to: None,
            },
        ];

        let mut expected = Changes::new();
        expected.0.insert(
            RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
            FileChanges::Changed {
                changes: vec![(12, 12), (2, 3)].into_iter().collect(),
                additions: vec![100].into_iter().collect(),
                deletions: vec![200].into_iter().collect(),
            },
        );

        let mut actual = Changes::new();
        actual.0.insert(
            RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
            FileChanges::new_changed(),
        );
        line_events.into_iter().for_each(|e| {
            actual.process_line(e);
        });

        assert_eq!(expected, actual);
    }

    #[test]
    fn mix_events() {
        let file_events = vec![
            DiffFileEvent {
                from: RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
                to: Some(RelativePathBuf::from("foo/bar/baz.txt".to_owned())),
            },
            DiffFileEvent {
                from: RelativePathBuf::from("foo/qux.txt".to_owned()),
                to: None,
            },
            DiffFileEvent {
                from: RelativePathBuf::from("hello/world.md".to_owned()),
                to: Some(RelativePathBuf::from("hello/world/doc.md".to_owned())),
            },
        ];

        let line_events = vec![
            DiffLineEvent {
                key: RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
                from: Some(100),
                to: None,
            },
            DiffLineEvent {
                key: RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
                from: None,
                to: Some(300),
            },
            DiffLineEvent {
                key: RelativePathBuf::from("hello/world.md".to_owned()),
                from: Some(100),
                to: Some(200),
            },
        ];

        let mut expected = Changes::new();
        expected.0.insert(
            RelativePathBuf::from("foo/bar/baz.txt".to_owned()),
            FileChanges::Changed {
                changes: vec![].into_iter().collect(),
                additions: vec![300].into_iter().collect(),
                deletions: vec![100].into_iter().collect(),
            },
        );
        expected.0.insert(
            RelativePathBuf::from("foo/qux.txt".to_owned()),
            FileChanges::Deleted,
        );
        expected.0.insert(
            RelativePathBuf::from("hello/world.md".to_owned()),
            FileChanges::Renamed {
                new_name: RelativePathBuf::from("hello/world/doc.md".to_owned()),
                changes: vec![(100, 200)].into_iter().collect(),
                additions: vec![].into_iter().collect(),
                deletions: vec![].into_iter().collect(),
            },
        );

        let mut actual = Changes::new();
        file_events.into_iter().for_each(|e| {
            actual.process_file(e);
        });
        line_events.into_iter().for_each(|e| {
            actual.process_line(e);
        });

        assert_eq!(expected, actual);
    }
}
