use crate::types::path::RelativePathBuf;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;

#[derive(Debug, PartialEq, Eq)]
pub struct Changes(HashMap<RelativePathBuf, FileChanges>);

impl Changes {
    pub fn new() -> Self {
        Changes(HashMap::new())
    }

    #[allow(dead_code)]
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
            if let Some(f) = from {
                match to {
                    None => m.line_deleted(f),
                    Some(t) => m.line_moved(f, t),
                };
            }
        });
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FileChanges {
    Deleted,
    Renamed {
        new_name: RelativePathBuf,
        changes: HashMap<usize, usize>,
        deletions: HashSet<usize>,
    },
    Changed {
        changes: HashMap<usize, usize>,
        deletions: HashSet<usize>,
    },
}

impl FileChanges {
    fn new_changed() -> Self {
        FileChanges::Changed {
            changes: HashMap::new(),
            deletions: HashSet::new(),
        }
    }

    fn new_renamed(path: RelativePathBuf) -> Self {
        FileChanges::Renamed {
            new_name: path,
            changes: HashMap::new(),
            deletions: HashSet::new(),
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
}

pub struct DiffFileEvent {
    pub from: RelativePathBuf,
    pub to: Option<RelativePathBuf>,
}

pub struct DiffLineEvent {
    pub key: RelativePathBuf,
    pub from: Option<u32>,
    pub to: Option<u32>,
}
