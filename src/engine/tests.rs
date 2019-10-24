use super::io::TourFileManager;
use super::*;
use crate::error::Result;
use crate::index::Index;
use crate::types::path::{AbsolutePath, AbsolutePathBuf, RelativePathBuf};
use crate::types::{Stop, StopReference, Tour};
use crate::vcs::{Changes, FileChanges, LineChanges, VCS};
use dirs;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone)]
pub struct MockTourFileManager {
    pub file_system: Rc<RefCell<HashMap<PathBuf, Tour>>>,
    pub path_map: HashMap<TourId, PathBuf>,
}

impl MockTourFileManager {
    pub fn new() -> Self {
        MockTourFileManager {
            file_system: Rc::new(RefCell::new(HashMap::new())),
            path_map: HashMap::new(),
        }
    }
}

impl TourFileManager for MockTourFileManager {
    fn save_tour(&self, tour: &Tour) -> Result<()> {
        let path = self.path_map.get(&tour.id).unwrap();
        self.file_system
            .borrow_mut()
            .insert(path.clone(), tour.clone());
        Ok(())
    }

    fn load_tour(&self, path: PathBuf) -> Result<Tour> {
        Ok(self.file_system.borrow().get(&path).unwrap().clone())
    }

    fn delete_tour(&mut self, tour_id: TourId) -> Result<()> {
        let path = self.path_map.remove(&tour_id).unwrap();
        self.file_system.borrow_mut().remove(&path);
        Ok(())
    }

    fn set_tour_path(&mut self, tour_id: TourId, path: PathBuf) {
        self.path_map.insert(tour_id, path);
    }

    fn reload_tour(&self, tour_id: TourId) -> Result<Tour> {
        let path = self.path_map.get(&tour_id).unwrap();
        Ok(self.file_system.borrow().get(path).unwrap().clone())
    }
}

#[derive(Clone)]
struct MockVCS {
    last_changes: Option<Changes>,
}

impl VCS for MockVCS {
    fn get_current_version(&self, _repo_path: AbsolutePath<'_>) -> Result<String> {
        Ok("COMMIT".to_owned())
    }

    fn diff_with_version(
        &self,
        _repo_path: AbsolutePath<'_>,
        _from: &str,
        _to: &str,
    ) -> Result<Changes> {
        Ok(self.last_changes.clone().unwrap())
    }

    fn is_workspace_dirty(&self, _repo_path: AbsolutePath<'_>) -> Result<bool> {
        Ok(false)
    }

    fn diff_with_worktree(&self, _repo_path: AbsolutePath<'_>, _from: &str) -> Result<Changes> {
        Ok(self.last_changes.clone().unwrap())
    }

    fn checkout_version(&self, _repo_version: AbsolutePath<'_>, to: &str) -> Result<String> {
        Ok(to.to_owned())
    }

    fn lookup_file_bytes(
        &self,
        _repo_path: AbsolutePath<'_>,
        _commit: &str,
        _file_path: &RelativePathBuf,
    ) -> Result<Vec<u8>> {
        panic!("No implementation needed yet. Add one if necessary.")
    }
}

#[derive(Clone)]
struct MockIndex(pub Rc<RefCell<HashMap<String, AbsolutePathBuf>>>);

impl Index for MockIndex {
    fn get(&self, repo_name: &str) -> Result<Option<AbsolutePathBuf>> {
        Ok(self.0.borrow().get(repo_name).cloned())
    }

    fn set(&self, repo_name: &str, path: &AbsolutePathBuf) -> Result<()> {
        self.0
            .borrow_mut()
            .insert(repo_name.to_owned(), path.clone());
        Ok(())
    }

    fn unset(&self, repo_name: &str) -> Result<()> {
        self.0.borrow_mut().remove(repo_name);
        Ok(())
    }

    fn all(&self) -> Result<Vec<(String, AbsolutePathBuf)>> {
        Ok(self
            .0
            .borrow()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect())
    }
}

fn test_instance() -> Engine<MockTourFileManager, MockVCS, MockIndex> {
    Engine {
        tours: HashMap::new(),
        manager: MockTourFileManager::new(),
        edits: HashSet::new(),
        vcs: MockVCS { last_changes: None },
        index: MockIndex(Rc::new(RefCell::new(HashMap::new()))),
    }
}

#[test]
fn list_tours_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
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
    let mut tourist = test_instance();
    let id = tourist
        .create_tour("My first tour".to_owned())
        .expect("Call to create failed");
    let tour = tourist.tours.get(&id).expect("Tour not found");
    assert_eq!(tour.id, id);
    assert_eq!(tour.title, "My first tour");
}

#[test]
fn open_tour_test() {
    let tour_file = PathBuf::from("some/path");

    let mut tourist = test_instance();

    tourist.manager.file_system.borrow_mut().insert(
        tour_file.clone(),
        Tour {
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
    let tour = tourist.tours.get("TOURID").expect("Tour not found");
    assert_eq!(tour.title, "My first tour");
    assert_eq!(tour.stops, vec![]);
}

#[test]
fn freeze_unfreeze_tour_test() {
    let mut tourist = test_instance();
    let tour = Tour {
        id: "TOURID".to_owned(),
        title: "My first tour".to_owned(),
        description: "".to_owned(),
        stops: vec![],
        protocol_version: "1.0".to_owned(),
        repositories: vec![].into_iter().collect(),
    };
    tourist.tours.insert("TOURID".to_owned(), tour.clone());

    tourist
        .manager
        .path_map
        .insert("TOURID".to_owned(), PathBuf::from("/foo/bar"));
    tourist
        .manager
        .file_system
        .borrow_mut()
        .insert(PathBuf::from("/foo/bar"), tour);

    tourist.unfreeze_tour("TOURID".to_owned()).unwrap();
    assert!(tourist.is_editable("TOURID"));
    tourist.freeze_tour("TOURID".to_owned()).unwrap();
    assert!(!tourist.is_editable("TOURID"));
}

#[test]
fn view_tour_test() {
    let mut tourist = test_instance();
    let root = dirs::download_dir().unwrap();
    tourist
        .index
        .set("my-repo", &AbsolutePathBuf::new(root.join("foo")).unwrap())
        .unwrap();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.vcs.last_changes = Some(Changes::new());

    let view = tourist.view_tour("TOURID".to_owned()).unwrap();
    assert_eq!(
        view,
        TourView {
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![("STOPID".to_owned(), "A stop on the tour".to_owned())],
            repositories: vec![("my-repo".to_owned(), "COMMIT".to_owned())],
            edit: false,
            up_to_date: true,
        }
    );
    tourist.set_editable("TOURID".to_owned(), true);
    assert!(tourist.view_tour("TOURID".to_owned()).unwrap().edit);
}

#[test]
fn edit_tour_metadata_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: vec![].into_iter().collect(),
        },
    );
    tourist.set_editable("TOURID".to_owned(), true);

    tourist
        .edit_tour_metadata(
            "TOURID".to_owned(),
            TourMetadata {
                title: Some("New title".to_owned()),
                description: None,
            },
        )
        .unwrap();
    assert_eq!(tourist.tours.get("TOURID").unwrap().title, "New title");

    tourist
        .edit_tour_metadata(
            "TOURID".to_owned(),
            TourMetadata {
                title: None,
                description: Some("A description".to_owned()),
            },
        )
        .unwrap();
    assert_eq!(tourist.tours.get("TOURID").unwrap().title, "New title");
    assert_eq!(
        tourist.tours.get("TOURID").unwrap().description,
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
    assert_eq!(tourist.tours.get("TOURID").unwrap().title, "X");
    assert_eq!(tourist.tours.get("TOURID").unwrap().description, "X");
}

#[test]
fn forget_tour_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: vec![].into_iter().collect(),
        },
    );
    tourist.forget_tour("TOURID".to_owned()).unwrap();
    assert!(tourist.tours.is_empty());
}

#[test]
fn reload_tour_test() {
    let mut tourist = test_instance();
    let tour = Tour {
        id: "TOURID".to_owned(),
        title: "My first tour".to_owned(),
        description: "".to_owned(),
        stops: vec![],
        protocol_version: "1.0".to_owned(),
        repositories: vec![].into_iter().collect(),
    };
    tourist.tours.insert("TOURID".to_owned(), tour.clone());

    tourist
        .manager
        .path_map
        .insert("TOURID".to_owned(), PathBuf::from("/foo/bar"));
    tourist
        .manager
        .file_system
        .borrow_mut()
        .insert(PathBuf::from("/foo/bar"), tour);

    tourist.set_editable("TOURID".to_owned(), true);
    tourist
        .edit_tour_metadata(
            "TOURID".to_owned(),
            TourMetadata {
                title: Some("Different name".to_owned()),
                description: None,
            },
        )
        .unwrap();
    tourist.reload_tour("TOURID".to_owned()).unwrap();

    assert_eq!(tourist.tours.get("TOURID").unwrap().title, "My first tour");
}

#[test]
fn create_stop_test() {
    let mut tourist = test_instance();
    let root = dirs::download_dir().unwrap();
    tourist
        .index
        .set("my-repo", &AbsolutePathBuf::new(root.join("foo")).unwrap())
        .unwrap();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![],
            protocol_version: "1.0".to_owned(),
            repositories: vec![].into_iter().collect(),
        },
    );
    tourist.set_editable("TOURID".to_owned(), true);
    let id = tourist
        .create_stop(
            "TOURID".to_owned(),
            "A tour stop".to_owned(),
            root.join("foo").join("bar").join("baz"),
            100,
        )
        .unwrap();

    let tours = tourist.tours;
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
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);
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
        let tour = tourist.tours.get("TOURID").unwrap();
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
        let tour = tourist.tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].title, "Something");
        assert_eq!(tour.stops[0].description, "Other thing");
    }
}

#[test]
fn move_stop_test() {
    let mut tourist = test_instance();
    tourist
        .index
        .set(
            "my-repo",
            &AbsolutePathBuf::new(PathBuf::from("/foo")).unwrap(),
        )
        .unwrap();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);

    tourist
        .move_stop(
            "TOURID".to_owned(),
            "STOPID".to_owned(),
            PathBuf::from("/foo/bar/baz.txt"),
            500,
        )
        .unwrap();

    let tours = tourist.tours;
    let tour = tours.get("TOURID").unwrap();
    assert_eq!(tour.stops[0].line, 500,);
    assert_eq!(
        tour.stops[0].path,
        RelativePathBuf::from("bar/baz.txt".to_owned())
    );
}

#[test]
fn reorder_stop_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![
                Stop {
                    broken: None,
                    id: "0".to_owned(),
                    title: "A stop on the tour".to_owned(),
                    description: "".to_owned(),
                    path: RelativePathBuf::from("foo/bar.txt".to_owned()),
                    repository: "my-repo".to_owned(),
                    line: 100,
                    children: vec![],
                },
                Stop {
                    broken: None,
                    id: "1".to_owned(),
                    title: "Another stop on the tour".to_owned(),
                    description: "".to_owned(),
                    path: RelativePathBuf::from("foo/baz.txt".to_owned()),
                    repository: "my-repo".to_owned(),
                    line: 200,
                    children: vec![],
                },
                Stop {
                    broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);

    {
        tourist
            .reorder_stop("TOURID".to_owned(), "0".to_owned(), 1)
            .unwrap();
        let tour = tourist.tours.get("TOURID").unwrap();
        assert_eq!(
            tour.stops.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "0", "2"]
        );
    }
    {
        tourist
            .reorder_stop("TOURID".to_owned(), "0".to_owned(), 5)
            .unwrap();
        let tour = tourist.tours.get("TOURID").unwrap();
        assert_eq!(
            tour.stops.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "2", "0"]
        );
    }
    {
        tourist
            .reorder_stop("TOURID".to_owned(), "0".to_owned(), -1)
            .unwrap();
        let tour = tourist.tours.get("TOURID").unwrap();
        assert_eq!(
            tour.stops.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["1", "0", "2"]
        );
    }
    {
        tourist
            .reorder_stop("TOURID".to_owned(), "0".to_owned(), -100)
            .unwrap();
        let tour = tourist.tours.get("TOURID").unwrap();
        assert_eq!(
            tour.stops.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            vec!["0", "1", "2"]
        );
    }
}

#[test]
fn link_stop_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);
    {
        tourist
            .link_stop(
                "TOURID".to_owned(),
                "STOPID".to_owned(),
                "OTHERID".to_owned(),
                None,
            )
            .unwrap();
        let tour = tourist.tours.get("TOURID").unwrap();
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
        let tour = tourist.tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].children[1].tour_id, "SECONDID");
        assert_eq!(
            tour.stops[0].children[1].stop_id,
            Some("SECONDSTOPID".to_owned())
        );
    }
}

#[test]
fn unlink_stop_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);
    {
        tourist
            .unlink_stop(
                "TOURID".to_owned(),
                "STOPID".to_owned(),
                "OTHERID".to_owned(),
                None,
            )
            .unwrap();
        let tour = tourist.tours.get("TOURID").unwrap();
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
        let tours = tourist.tours;
        let tour = tours.get("TOURID").unwrap();
        assert_eq!(tour.stops[0].children.len(), 0);
    }
}

#[test]
fn locate_stop_test() {
    let mut tourist = test_instance();
    let root = dirs::download_dir().unwrap();
    tourist
        .index
        .set("my-repo", &AbsolutePathBuf::new(root.join("foo")).unwrap())
        .unwrap();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    assert_eq!(path, root.join("foo").join("bar").join("baz.txt"));
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
    assert_eq!(path, root.join("foo").join("bar").join("baz.txt"));
    assert_eq!(line, 105);
}

#[test]
fn remove_stop_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);

    tourist
        .remove_stop("TOURID".to_owned(), "STOPID".to_owned())
        .unwrap();
    let tours = tourist.tours;
    let tour = tours.get("TOURID").unwrap();
    assert_eq!(tour.stops.len(), 0);
}

#[test]
fn refresh_tour_test() {
    let mut tourist = test_instance();
    let root = dirs::download_dir().unwrap();
    tourist
        .index
        .set("my-repo", &AbsolutePathBuf::new(root.join("foo")).unwrap())
        .unwrap();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);

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

    tourist.refresh_tour("TOURID".to_owned()).unwrap();

    let tours = tourist.tours;
    let tour = tours.get("TOURID").unwrap();
    assert_eq!(tour.stops[0].line, 105);
    assert_eq!(tour.repositories.get("my-repo").unwrap(), "COMMIT");
}

#[test]
fn save_tour_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);

    let path = PathBuf::from("/foo/bar");

    tourist
        .save_tour("TOURID".to_owned(), Some(path.clone()))
        .unwrap();

    assert_eq!(
        tourist.manager.file_system.borrow().get(&path).unwrap().id,
        "TOURID"
    );
}

#[test]
fn delete_tour_test() {
    let mut tourist = test_instance();
    tourist.tours.insert(
        "TOURID".to_owned(),
        Tour {
            id: "TOURID".to_owned(),
            title: "My first tour".to_owned(),
            description: "".to_owned(),
            stops: vec![Stop {
                broken: None,
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
    tourist.set_editable("TOURID".to_owned(), true);

    let path = PathBuf::from("/foo/bar");

    tourist
        .save_tour("TOURID".to_owned(), Some(path.clone()))
        .unwrap();
    tourist.delete_tour("TOURID".to_owned()).unwrap();

    assert!(tourist.manager.file_system.borrow().get(&path).is_none());
}

#[test]
fn index_repository_test() {
    let mut tourist = test_instance();
    let root = dirs::download_dir().unwrap();
    tourist
        .index_repository("my-repo".to_owned(), Some(root.join("foo")))
        .unwrap();
    assert_eq!(
        tourist.index.get("my-repo").unwrap().unwrap(),
        AbsolutePathBuf::new(root.join("foo")).unwrap()
    );
}
