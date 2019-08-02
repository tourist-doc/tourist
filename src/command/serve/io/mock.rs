use super::super::error::Result;
use super::super::interface::TourId;
use super::TourFileManager;
use crate::types::Tour;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct MockTourFileManager {
    pub file_system: Arc<RwLock<HashMap<PathBuf, Tour>>>,
    pub tour_map: HashMap<TourId, Tour>,
    pub path_map: HashMap<TourId, PathBuf>,
}

impl MockTourFileManager {
    pub fn new() -> Self {
        MockTourFileManager {
            file_system: Arc::new(RwLock::new(HashMap::new())),
            tour_map: HashMap::new(),
            path_map: HashMap::new(),
        }
    }
}

impl TourFileManager for MockTourFileManager {
    fn save_tour(&self, tour_id: TourId) -> Result<()> {
        let tour = self.tour_map.get(&tour_id).unwrap();
        let path = self.path_map.get(&tour_id).unwrap();
        self.file_system
            .write()
            .unwrap()
            .insert(path.clone(), tour.clone());
        Ok(())
    }

    fn load_tour(&self, path: PathBuf) -> Result<Tour> {
        Ok(self.file_system.read().unwrap().get(&path).unwrap().clone())
    }
}
