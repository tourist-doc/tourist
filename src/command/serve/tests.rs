use super::{
    Result, StopMetadata, StopView, TourFileManager, TourId, TourMetadata, TourView, Tourist,
    TouristRpc,
};
use crate::error;
use crate::index::Index;
use crate::types::path::{AbsolutePath, AbsolutePathBuf, RelativePathBuf};
use crate::types::{Stop, StopReference, Tour};
use crate::vcs::{Changes, FileChanges, LineChanges, VCS};
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
    pub fn new(tour_map: Arc<RwLock<HashMap<TourId, Tour>>>) -> Self {
        MockTourFileManager {
            file_system: Arc::new(RwLock::new(HashMap::new())),
            path_map: Arc::new(RwLock::new(HashMap::new())),
            tour_map,
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
        let path = self.path_map.write().unwrap().remove(&tour_id).unwrap();
        self.file_system.write().unwrap().remove(&path);
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
        panic!("No implementation needed yet. Add one if necessary.")
    }
}

#[derive(Clone)]
struct MockIndex(pub Arc<RwLock<HashMap<String, AbsolutePathBuf>>>);

impl Index for MockIndex {
    fn get(&self, repo_name: &str) -> Result<Option<AbsolutePathBuf>> {
        Ok(self.0.read().unwrap().get(repo_name).cloned())
    }

    fn set(&self, repo_name: &str, path: &AbsolutePathBuf) -> Result<()> {
        self.0
            .write()
            .unwrap()
            .insert(repo_name.to_owned(), path.clone());
        Ok(())
    }

    fn unset(&self, repo_name: &str) -> Result<()> {
        self.0.write().unwrap().remove(repo_name);
        Ok(())
    }

    fn all(&self) -> Result<Vec<(String, AbsolutePathBuf)>> {
        Ok(self
            .0
            .read()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect())
    }
}

fn test_instance() -> (
    Tourist<MockTourFileManager, MockVCS, MockIndex>,
    MockTourFileManager,
    MockIndex,
) {
    let tours = Arc::new(RwLock::new(HashMap::new()));
    let manager = MockTourFileManager::new(Arc::clone(&tours));
    let index = MockIndex(Arc::new(RwLock::new(HashMap::new())));
    (
        Tourist {
            tours,
            manager: manager.clone(),
            vcs: MockVCS { last_changes: None },
            index: index.clone(),
            edits: Arc::new(RwLock::new(HashSet::new())),
        },
        manager,
        index,
    )
}

#[test]
fn list_tours_test() {
    let (tourist, _, _) = test_instance();
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
    let (tourist, _, _) = test_instance();
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

    let (tourist, manager, _) = test_instance();

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
    let (tourist, _, _) = test_instance();
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
    let (tourist, _, _) = test_instance();
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
    let (tourist, _, _) = test_instance();
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
    let (tourist, _, _) = test_instance();
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
    let (tourist, _, index) = test_instance();
    index
        .set(
            "my-repo",
            &AbsolutePathBuf::new(PathBuf::from("/foo")).unwrap(),
        )
        .unwrap();
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
    let (tourist, _, _) = test_instance();
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
    let (tourist, _, _) = test_instance();
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
fn move_stop_test() {
    let (tourist, _, index) = test_instance();
    index
        .set(
            "my-repo",
            &AbsolutePathBuf::new(PathBuf::from("/foo")).unwrap(),
        )
        .unwrap();
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
        .move_stop(
            "TOURID".to_owned(),
            "STOPID".to_owned(),
            PathBuf::from("/foo/bar/baz.txt"),
            500,
        )
        .unwrap();

    let tours = tourist.get_tours();
    let tour = tours.get("TOURID").unwrap();
    assert_eq!(tour.stops[0].line, 500,);
    assert_eq!(
        tour.stops[0].path,
        RelativePathBuf::from("bar/baz.txt".to_owned())
    );
}

#[test]
fn reorder_stop_test() {
    let (tourist, _, _) = test_instance();
    tourist.get_tours_mut().insert(
        "TOURID".to_owned(),
        Tour {
            generator: 0,
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![
                Stop {
                    id: "0".to_owned(),
                    title: "A stop on the tour".to_owned(),
                    description: "".to_owned(),
                    path: RelativePathBuf::from("foo/bar.txt".to_owned()),
                    repository: "my-repo".to_owned(),
                    line: 100,
                    children: vec![],
                },
                Stop {
                    id: "1".to_owned(),
                    title: "Another stop on the tour".to_owned(),
                    description: "".to_owned(),
                    path: RelativePathBuf::from("foo/baz.txt".to_owned()),
                    repository: "my-repo".to_owned(),
                    line: 200,
                    children: vec![],
                },
                Stop {
                    id: "2".to_owned(),
                    title: "A third stop on the tour".to_owned(),
                    description: "".to_owned(),
                    path: RelativePathBuf::from("foo/qux.txt".to_owned()),
                    repository: "my-repo".to_owned(),
                    line: 300,
                    children: vec![],
                },
            ],
            protocol_version: "1.0".to_owned(),
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );

    {
        tourist
            .reorder_stop("TOURID".to_owned(), "0".to_owned(), 1)
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(
            tour.stops.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "0", "2"]
        );
    }
    {
        tourist
            .reorder_stop("TOURID".to_owned(), "0".to_owned(), 5)
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(
            tour.stops.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "2", "0"]
        );
    }
    {
        tourist
            .reorder_stop("TOURID".to_owned(), "0".to_owned(), -1)
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(
            tour.stops.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "0", "2"]
        );
    }
    {
        tourist
            .reorder_stop("TOURID".to_owned(), "0".to_owned(), -100)
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(
            tour.stops.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["0", "1", "2"]
        );
    }
}

#[test]
fn link_stop_test() {
    let (tourist, _, _) = test_instance();
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
fn unlink_stop_test() {
    let (tourist, _, _) = test_instance();
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
                children: vec![
                    StopReference {
                        tour_id: "OTHERID".to_owned(),
                        stop_id: None,
                    },
                    StopReference {
                        tour_id: "SECONDID".to_owned(),
                        stop_id: Some("SECONDSTOPID".to_owned()),
                    },
                ],
            }],
            protocol_version: "1.0".to_owned(),
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );
    {
        tourist
            .unlink_stop(
                "TOURID".to_owned(),
                "STOPID".to_owned(),
                "OTHERID".to_owned(),
                None,
            )
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].children.len(), 1);
    }
    {
        tourist
            .unlink_stop(
                "TOURID".to_owned(),
                "STOPID".to_owned(),
                "SECONDID".to_owned(),
                Some("SECONDSTOPID".to_owned()),
            )
            .unwrap();
        let tours = tourist.get_tours();
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].children.len(), 0);
    }
}

#[test]
fn locate_stop_test() {
    let (mut tourist, _, index) = test_instance();
    index
        .set(
            "my-repo",
            &AbsolutePathBuf::new(PathBuf::from("/foo")).unwrap(),
        )
        .unwrap();
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
            line_changes: LineChanges {
                changes: vec![(100, 105)].into_iter().collect(),
                deletions: vec![].into_iter().collect(),
                additions: vec![].into_iter().collect(),
            },
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
    let (tourist, _, _) = test_instance();
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

#[test]
fn refresh_tour_test() {
    let (mut tourist, _, index) = test_instance();
    index
        .set(
            "my-repo",
            &AbsolutePathBuf::new(PathBuf::from("/foo")).unwrap(),
        )
        .unwrap();
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
            repositories: vec![("my-repo".to_owned(), "OLD_COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );

    let mut changes = Changes::new();
    changes.0.insert(
        RelativePathBuf::from("foo/bar.txt".to_owned()),
        FileChanges::Changed {
            line_changes: LineChanges {
                changes: vec![(100, 105)].into_iter().collect(),
                deletions: vec![].into_iter().collect(),
                additions: vec![].into_iter().collect(),
            },
        },
    );
    tourist.vcs.last_changes = Some(changes);

    tourist
        .refresh_tour("TOURID".to_owned(), Some("NEW_COMMIT".to_owned()))
        .unwrap();

    let tours = tourist.get_tours();
    let tour = tours.get("TOURID").unwrap();
    assert_eq!(tour.stops[0].line, 105);
    assert_eq!(tour.repositories.get("my-repo").unwrap(), "NEW_COMMIT");
}

#[test]
fn save_tour_test() {
    let (tourist, manager, _) = test_instance();
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
            repositories: vec![("my-repo".to_owned(), "OLD_COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );

    let path = PathBuf::from("/foo/bar");

    tourist
        .save_tour("TOURID".to_owned(), Some(path.clone()))
        .unwrap();

    let fs = manager.file_system.read().unwrap();
    assert_eq!(fs.get(&path).unwrap().id, "TOURID");
}

#[test]
fn save_all_test() {
    let (tourist, manager, _) = test_instance();
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
            repositories: vec![("my-repo".to_owned(), "OLD_COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );

    manager
        .path_map
        .write()
        .unwrap()
        .insert("TOURID".to_owned(), PathBuf::from("/foo/bar"));

    tourist.save_all().unwrap();

    let fs = manager.file_system.read().unwrap();
    assert_eq!(fs.get(&PathBuf::from("/foo/bar")).unwrap().id, "TOURID");
}

#[test]
fn delete_tour_test() {
    let (tourist, manager, _) = test_instance();
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
            repositories: vec![("my-repo".to_owned(), "OLD_COMMIT".to_owned())]
                .into_iter()
                .collect(),
        },
    );

    let path = PathBuf::from("/foo/bar");

    tourist
        .save_tour("TOURID".to_owned(), Some(path.clone()))
        .unwrap();
    tourist.delete_tour("TOURID".to_owned()).unwrap();

    let fs = manager.file_system.read().unwrap();
    assert!(fs.get(&path).is_none());
}

#[test]
fn index_repository_test() {
    let (tourist, _, index) = test_instance();
    tourist
        .index_repository("my-repo".to_owned(), Some(PathBuf::from("/foo/bar")))
        .unwrap();
    assert_eq!(
        index.get("my-repo").unwrap().unwrap(),
        AbsolutePathBuf::new(PathBuf::from("/foo/bar")).unwrap()
    );
}
