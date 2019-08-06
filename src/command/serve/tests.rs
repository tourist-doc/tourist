use super::{Result, TourFileManager, TourId, Tourist, TouristRpc};
use crate::error;
use crate::types::path::{AbsolutePath, RelativePathBuf};
use crate::types::Tour;
use crate::vcs::{Changes, VCS};
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
struct MockVCS;

impl VCS for MockVCS {
    fn get_current_version(&self, _repo_path: AbsolutePath<'_>) -> error::Result<String> {
        unimplemented!();
    }

    fn diff_with_version(
        &self,
        _repo_path: AbsolutePath<'_>,
        _from: &str,
        _to: &str,
    ) -> error::Result<Changes> {
        unimplemented!();
    }

    fn diff_with_worktree(
        &self,
        _repo_path: AbsolutePath<'_>,
        _from: &str,
    ) -> error::Result<Changes> {
        unimplemented!();
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
            vcs: MockVCS,
            tours: Arc::new(RwLock::new(HashMap::new())),
            index: Arc::new(RwLock::new(HashMap::new())),
            edits: Arc::new(RwLock::new(HashSet::new())),
        },
        manager,
    )
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
