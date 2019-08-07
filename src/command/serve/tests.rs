use super::{
    Result, StopMetadata, StopView, TourFileManager, TourId, TourMetadata, TourView, Tourist,
    TouristRpc,
};
use crate::error;
use crate::types::path::{AbsolutePath, AbsolutePathBuf, RelativePathBuf};
use crate::types::{Stop, Tour};
use crate::vcs::{Changes, FileChanges, VCS};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct MockTourFileManager {
    pub file_system: Arc<RwLock<HashMap<PathBuf, Tour>>>,
    pub path_map: Arc<RwLock<HashMap<TourId, PathBuf>>>,
    pub tour_map: Arc<RwLock<HashMap<TourId, Tour>>>,
}

impl MockTourFileManager {
    pub fn new() -> Self {
        MockTourFileManager {
            file_system: Arc::new(RwLock::new(HashMap::new())),
            path_map: Arc::new(RwLock::new(HashMap::new())),
            tour_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl TourFileManager for MockTourFileManager {
    fn save_tour(&self, tour_id: TourId) -> Result<()> {
        let tours = self.tour_map.read().unwrap();
        let tour = tours.get(&tour_id).unwrap();
        let paths = self.path_map.read().unwrap();
        let path = paths.get(&tour_id).unwrap();
        self.file_system
            .write()
            .unwrap()
            .insert(path.clone(), tour.clone());
        Ok(())
    }

    fn load_tour(&self, path: PathBuf) -> Result<Tour> {
        Ok(self.file_system.read().unwrap().get(&path).unwrap().clone())
    }

    fn delete_tour(&self, tour_id: TourId) -> Result<()> {
        self.tour_map.write().unwrap().remove(&tour_id);
        self.path_map.write().unwrap().remove(&tour_id);
        Ok(())
    }

    fn set_tour_path(&self, tour_id: TourId, path: PathBuf) {
        let mut paths = self.path_map.write().unwrap();
        paths.insert(tour_id, path);
    }
}

#[derive(Clone)]
struct MockVCS {
    last_changes: Option<Changes>,
}

impl VCS for MockVCS {
    fn get_current_version(&self, _repo_path: AbsolutePath<'_>) -> error::Result<String> {
        Ok("COMMIT".to_owned())
    }

    fn diff_with_version(
        &self,
        _repo_path: AbsolutePath<'_>,
        _from: &str,
        _to: &str,
    ) -> error::Result<Changes> {
        Ok(self.last_changes.clone().unwrap())
    }

    fn diff_with_worktree(
        &self,
        _repo_path: AbsolutePath<'_>,
        _from: &str,
    ) -> error::Result<Changes> {
        Ok(self.last_changes.clone().unwrap())
    }

    fn lookup_file_bytes(
        &self,
        _repo_path: AbsolutePath<'_>,
        _commit: &str,
        _file_path: &RelativePathBuf,
    ) -> error::Result<Vec<u8>> {
        unimplemented!();
    }
}

fn test_instance() -> (Tourist<MockTourFileManager, MockVCS>, MockTourFileManager) {
    let manager = MockTourFileManager::new();
    (
        Tourist {
            manager: manager.clone(),
            vcs: MockVCS { last_changes: None },
            tours: Arc::new(RwLock::new(HashMap::new())),
            index: Arc::new(RwLock::new(HashMap::new())),
            edits: Arc::new(RwLock::new(HashSet::new())),
        },
        manager,
    )
}

#[test]
fn list_tours_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: vec![].into_iter().collect(),
        },
    );
    assert_eq!(
        tourist.list_tours().unwrap(),
        vec![("TOURID".to_owned(), "My first tour".to_owned())]
    );
}

#[test]
fn create_tour_test() {
    let (tourist, _) = test_instance();
    let id = tourist
        .create_tour("My first tour".to_owned())
        .expect("Call to create failed");
    let tours = tourist.tours.read().expect("Lock was poisoned");
    let tour = tours.get(&id).expect("Tour not found");
    assert_eq!(tour.id, id);
    assert_eq!(tour.title, "My first tour");
}

#[test]
fn open_tour_test() {
    let tour_file = PathBuf::from("some/path");

    let (tourist, manager) = test_instance();

    manager.file_system.write().unwrap().insert(
        tour_file.clone(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: HashMap::new(),
        },
    );

    tourist
        .open_tour(tour_file, false)
        .expect("Call to open failed");
    let tours = tourist.tours.read().expect("Lock was poisoned");
    let tour = tours.get("TOURID").expect("Tour not found");
    assert_eq!(tour.title, "My first tour");
    assert_eq!(tour.stops, vec![]);
}

#[test]
fn set_tour_edit_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: vec![].into_iter().collect(),
        },
    );
    tourist.set_tour_edit("TOURID".to_owned(), true).unwrap();
    assert!(tourist.get_edits().contains("TOURID"));
    tourist.set_tour_edit("TOURID".to_owned(), false).unwrap();
    assert!(!tourist.get_edits().contains("TOURID"));
}

#[test]
fn view_tour_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                id: "STOPID".to_owned(),
                title: "A stop on the tour".to_owned(),
                description: "".to_owned(),
                path: RelativePathBuf::from("foo/bar.txt".to_owned()),
                repository: "my-repo".to_owned(),
                line: 100,
                children: vec![],
            }],
            protocol_version: "1.0".to_owned(),
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );
    let view = tourist.view_tour("TOURID".to_owned()).unwrap();
    assert_eq!(
        view,
        TourView {
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![("STOPID".to_owned(), "A stop on the tour".to_owned())],
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())],
            edit: false,
        }
    );
    tourist.get_edits_mut().insert("TOURID".to_owned());
    assert!(tourist.view_tour("TOURID".to_owned()).unwrap().edit);
}

#[test]
fn edit_tour_metadata_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: vec![].into_iter().collect(),
        },
    );

    tourist
        .edit_tour_metadata(
            "TOURID".to_owned(),
            TourMetadata {
                title: Some("New title".to_owned()),
                description: None,
            },
        )
        .unwrap();
    assert_eq!(
        tourist.get_tours().get("TOURID").unwrap().title,
        "New title"
    );

    tourist
        .edit_tour_metadata(
            "TOURID".to_owned(),
            TourMetadata {
                title: None,
                description: Some("A description".to_owned()),
            },
        )
        .unwrap();
    assert_eq!(
        tourist.get_tours().get("TOURID").unwrap().title,
        "New title"
    );
    assert_eq!(
        tourist.get_tours().get("TOURID").unwrap().description,
        "A description"
    );

    tourist
        .edit_tour_metadata(
            "TOURID".to_owned(),
            TourMetadata {
                title: Some("X".to_owned()),
                description: Some("X".to_owned()),
            },
        )
        .unwrap();
    assert_eq!(tourist.get_tours().get("TOURID").unwrap().title, "X");
    assert_eq!(tourist.get_tours().get("TOURID").unwrap().description, "X");
}

#[test]
fn forget_tour_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: vec![].into_iter().collect(),
        },
    );
    tourist.forget_tour("TOURID".to_owned()).unwrap();
    assert!(tourist.get_tours().is_empty());
}

#[test]
fn create_stop_test() {
    let (tourist, _) = test_instance();
    tourist.get_index_mut().insert(
        "my-repo".to_owned(),
        AbsolutePathBuf::new(PathBuf::from("/foo")).unwrap(),
    );
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: vec![].into_iter().collect(),
        },
    );
    let id = tourist
        .create_stop(
            "TOURID".to_owned(),
            "A tour stop".to_owned(),
            PathBuf::from("/foo/bar/baz"),
            100,
        )
        .unwrap();

    let tours = tourist.get_tours();
    let tour = tours.get("TOURID").unwrap();
    assert_eq!(tour.stops[0].id, id);
    assert_eq!(
        tour.stops[0].path,
        RelativePathBuf::from("bar/baz".to_owned())
    );
    assert_eq!(tour.repositories.get("my-repo").unwrap(), "COMMIT");
}

#[test]
fn view_stop_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                id: "STOPID".to_owned(),
                title: "A stop on the tour".to_owned(),
                description: "".to_owned(),
                path: RelativePathBuf::from("foo/bar.txt".to_owned()),
                repository: "my-repo".to_owned(),
                line: 100,
                children: vec![],
            }],
            protocol_version: "1.0".to_owned(),
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );
    let view = tourist
        .view_stop("TOURID".to_owned(), "STOPID".to_owned())
        .unwrap();
    assert_eq!(
        view,
        StopView {
            title: "A stop on the tour".to_owned(),
            description: "".to_owned(),
            repository: "my-repo".to_owned(),
            children: vec![],
        }
    );
}

#[test]
fn edit_stop_metadata_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                id: "STOPID".to_owned(),
                title: "A stop on the tour".to_owned(),
                description: "".to_owned(),
                path: RelativePathBuf::from("foo/bar.txt".to_owned()),
                repository: "my-repo".to_owned(),
                line: 100,
                children: vec![],
            }],
            protocol_version: "1.0".to_owned(),
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );
    {
        tourist
            .edit_stop_metadata(
                "TOURID".to_owned(),
                "STOPID".to_owned(),
                StopMetadata {
                    title: Some("Something".to_owned()),
                    description: None,
                },
            )
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].title, "Something");
    }
    {
        tourist
            .edit_stop_metadata(
                "TOURID".to_owned(),
                "STOPID".to_owned(),
                StopMetadata {
                    title: None,
                    description: Some("Other thing".to_owned()),
                },
            )
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].title, "Something");
        assert_eq!(tour.stops[0].description, "Other thing");
    }
}

#[test]
fn link_stop_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                id: "STOPID".to_owned(),
                title: "A stop on the tour".to_owned(),
                description: "".to_owned(),
                path: RelativePathBuf::from("foo/bar.txt".to_owned()),
                repository: "my-repo".to_owned(),
                line: 100,
                children: vec![],
            }],
            protocol_version: "1.0".to_owned(),
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );
    {
        tourist
            .link_stop(
                "TOURID".to_owned(),
                "STOPID".to_owned(),
                "OTHERID".to_owned(),
                None,
            )
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].children[0].tour_id, "OTHERID");
        assert_eq!(tour.stops[0].children[0].stop_id, None);
    }
    {
        tourist
            .link_stop(
                "TOURID".to_owned(),
                "STOPID".to_owned(),
                "SECONDID".to_owned(),
                Some("SECONDSTOPID".to_owned()),
            )
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].children[1].tour_id, "SECONDID");
        assert_eq!(
            tour.stops[0].children[1].stop_id,
            Some("SECONDSTOPID".to_owned())
        );
    }
}

#[test]
fn locate_stop_test() {
    let (mut tourist, _) = test_instance();
    tourist.get_index_mut().insert(
        "my-repo".to_owned(),
        AbsolutePathBuf::new(PathBuf::from("/foo")).unwrap(),
    );
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                id: "STOPID".to_owned(),
                title: "A stop on the tour".to_owned(),
                description: "".to_owned(),
                path: RelativePathBuf::from("bar/baz.txt".to_owned()),
                repository: "my-repo".to_owned(),
                line: 100,
                children: vec![],
            }],
            protocol_version: "1.0".to_owned(),
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );
    let (path, line) = tourist
        .locate_stop("TOURID".to_owned(), "STOPID".to_owned(), true)
        .unwrap()
        .unwrap();
    assert_eq!(path, PathBuf::from("/foo/bar/baz.txt"));
    assert_eq!(line, 100);

    let mut changes = Changes::new();
    changes.0.insert(
        RelativePathBuf::from("bar/baz.txt".to_owned()),
        FileChanges::Changed {
            changes: vec![(100, 105)].into_iter().collect(),
            deletions: vec![].into_iter().collect(),
            additions: vec![].into_iter().collect(),
        },
    );
    tourist.vcs.last_changes = Some(changes);
    let (path, line) = tourist
        .locate_stop("TOURID".to_owned(), "STOPID".to_owned(), false)
        .unwrap()
        .unwrap();
    assert_eq!(path, PathBuf::from("/foo/bar/baz.txt"));
    assert_eq!(line, 105);
}

#[test]
fn remove_stop_test() {
    let (tourist, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                id: "STOPID".to_owned(),
                title: "A stop on the tour".to_owned(),
                description: "".to_owned(),
                path: RelativePathBuf::from("foo/bar.txt".to_owned()),
                repository: "my-repo".to_owned(),
                line: 100,
                children: vec![],
            }],
            protocol_version: "1.0".to_owned(),
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );
    tourist
        .remove_stop("TOURID".to_owned(), "STOPID".to_owned())
        .unwrap();
    let tours = tourist.get_tours();
    let tour = tours.get("TOURID").unwrap();
    assert_eq!(tour.stops.len(), 0);
}
