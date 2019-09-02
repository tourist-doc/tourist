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

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Changes(pub HashMap<RelativePathBuf, FileChanges>);

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

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LineChanges {
    pub changes: HashMap<usize, usize>,
    pub additions: BTreeSet<usize>,
    pub deletions: BTreeSet<usize>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FileChanges {
    Deleted,
    Renamed {
        new_name: RelativePathBuf,
        line_changes: LineChanges,
    },
    Changed {
        line_changes: LineChanges,
    },
}

impl FileChanges {
    fn new_changed() -> Self {
        FileChanges::Changed {
            line_changes: LineChanges {
                changes: HashMap::new(),
                additions: BTreeSet::new(),
                deletions: BTreeSet::new(),
            },
        }
    }

    fn new_renamed(path: RelativePathBuf) -> Self {
        FileChanges::Renamed {
            new_name: path,
            line_changes: LineChanges {
                changes: HashMap::new(),
                additions: BTreeSet::new(),
                deletions: BTreeSet::new(),
            },
        }
    }

    fn line_moved(&mut self, from: usize, to: usize) {
        match self {
            FileChanges::Renamed { line_changes, .. } => {
                line_changes.changes.insert(from, to);
            }
            FileChanges::Changed { line_changes, .. } => {
                line_changes.changes.insert(from, to);
            }
            FileChanges::Deleted => {}
        }
    }

    fn line_added(&mut self, to: usize) {
        match self {
            FileChanges::Renamed { line_changes, .. } => {
                line_changes.additions.insert(to);
            }
            FileChanges::Changed { line_changes, .. } => {
                line_changes.additions.insert(to);
            }
            FileChanges::Deleted => {}
        }
    }

    fn line_deleted(&mut self, from: usize) {
        match self {
            FileChanges::Renamed { line_changes, .. } => {
                line_changes.deletions.insert(from);
            }
            FileChanges::Changed { line_changes, .. } => {
                line_changes.deletions.insert(from);
            }
            FileChanges::Deleted => {}
        }
    }

    pub fn adjust_line(&self, line: usize) -> Option<usize> {
        let lc = match self {
            FileChanges::Deleted => return None,
            FileChanges::Changed { line_changes } => line_changes,
            FileChanges::Renamed { line_changes, .. } => line_changes,
        };
        if lc.deletions.contains(&line) {
            return None;
        }
        if let Some(&dest) = lc.changes.get(&line) {
            return Some(dest);
        }
        let mut dest = line - lc.deletions.iter().filter(|x| **x < line).count();
        // Iterate over additions in ascending order
        for added_line in lc.additions.iter() {
            if *added_line <= dest {
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
    use super::{Changes, DiffFileEvent, DiffLineEvent, FileChanges, LineChanges};
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
                line_changes: LineChanges {
                    changes: vec![(12, 12), (2, 3)].into_iter().collect(),
                    additions: vec![100].into_iter().collect(),
                    deletions: vec![200].into_iter().collect(),
                },
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
                line_changes: LineChanges {
                    changes: vec![].into_iter().collect(),
                    additions: vec![300].into_iter().collect(),
                    deletions: vec![100].into_iter().collect(),
                },
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
                line_changes: LineChanges {
                    changes: vec![(100, 200)].into_iter().collect(),
                    additions: vec![].into_iter().collect(),
                    deletions: vec![].into_iter().collect(),
                },
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

    #[test]
    fn adjust_line_deleted() {
        for line in &[1, 2, 5, 100, 5000] {
            {
                let changes = FileChanges::Deleted;
                assert!(changes.adjust_line(*line).is_none());
            }
            {
                let changes = FileChanges::Changed {
                    line_changes: LineChanges {
                        changes: vec![].into_iter().collect(),
                        additions: vec![].into_iter().collect(),
                        deletions: vec![*line].into_iter().collect(),
                    },
                };
                assert!(changes.adjust_line(*line).is_none());
            }
        }
    }

    #[test]
    fn adjust_line_direct() {
        for line in &[1, 2, 5, 100, 5000] {
            {
                let changes = FileChanges::Changed {
                    line_changes: LineChanges {
                        changes: vec![(*line, 42)].into_iter().collect(),
                        additions: vec![].into_iter().collect(),
                        deletions: vec![].into_iter().collect(),
                    },
                };
                assert_eq!(changes.adjust_line(*line), Some(42));
            }
            {
                let changes = FileChanges::Renamed {
                    line_changes: LineChanges {
                        changes: vec![(*line, 42)].into_iter().collect(),
                        additions: vec![].into_iter().collect(),
                        deletions: vec![].into_iter().collect(),
                    },
                    new_name: RelativePathBuf::from("some/path".to_owned()),
                };
                assert_eq!(changes.adjust_line(*line), Some(42));
            }
        }
    }

    #[test]
    fn adjust_line_complex_additions_above() {
        // Before
        //   1 foo
        //   2 bar
        //   3 baz

        // After
        //   1 qux
        //   2
        //   3 foo
        //   4 bar
        //   5 baz

        let changes = FileChanges::Changed {
            line_changes: LineChanges {
                changes: vec![(1, 3)].into_iter().collect(),
                additions: vec![1, 2].into_iter().collect(),
                deletions: vec![].into_iter().collect(),
            },
        };
        assert_eq!(changes.adjust_line(1), Some(3));
        assert_eq!(changes.adjust_line(2), Some(4));
        assert_eq!(changes.adjust_line(3), Some(5));
    }

    #[test]
    fn adjust_line_complex_deletions_above() {
        // Before
        //   1 qux
        //   2
        //   3 foo
        //   4 bar
        //   5 baz
        //   6 last

        // After
        //   1 foo
        //   2 bar
        //   3 baz

        let changes = FileChanges::Changed {
            line_changes: LineChanges {
                changes: vec![(3, 1)].into_iter().collect(),
                additions: vec![].into_iter().collect(),
                deletions: vec![1, 2, 6].into_iter().collect(),
            },
        };
        assert_eq!(changes.adjust_line(1), None);
        assert_eq!(changes.adjust_line(2), None);
        assert_eq!(changes.adjust_line(3), Some(1));
        assert_eq!(changes.adjust_line(4), Some(2));
        assert_eq!(changes.adjust_line(5), Some(3));
        assert_eq!(changes.adjust_line(6), None);
    }

    #[test]
    fn adjust_line_complex_add_and_delete_above() {
        // Before
        //   1 qux
        //   2
        //   3 foo
        //   4 bar
        //   5 baz
        //   6 last

        // After
        //   1 yikes
        //   2 boo
        //   3 foo
        //   4 bar
        //   5 baz

        let changes = FileChanges::Changed {
            line_changes: LineChanges {
                changes: vec![].into_iter().collect(),
                additions: vec![1, 2].into_iter().collect(),
                deletions: vec![1, 2, 6].into_iter().collect(),
            },
        };
        assert_eq!(changes.adjust_line(1), None);
        assert_eq!(changes.adjust_line(2), None);
        assert_eq!(changes.adjust_line(3), Some(3));
        assert_eq!(changes.adjust_line(4), Some(4));
        assert_eq!(changes.adjust_line(5), Some(5));
        assert_eq!(changes.adjust_line(6), None);
    }
}
