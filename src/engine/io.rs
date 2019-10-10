use super::TourId;
use super::Tracker;
use crate::error::{ErrorKind, Result};
use crate::serialize;
use crate::types::Tour;
use failure::ResultExt;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub trait TourFileManager: Send + Sync + 'static {
    fn save_tour(&self, tour_id: TourId) -> Result<()>;
    fn load_tour(&self, path: PathBuf) -> Result<Tour>;
    fn delete_tour(&self, tour_id: TourId) -> Result<()>;
    fn set_tour_path(&self, tour_id: TourId, path: PathBuf);
    fn reload_tour(&self, tour_id: TourId) -> Result<Tour>;
}

pub struct AsyncTourFileManager {
    tours: Arc<RwLock<Tracker>>,
    paths: Arc<RwLock<HashMap<TourId, PathBuf>>>,
}

impl AsyncTourFileManager {
    pub fn new(tours: Arc<RwLock<Tracker>>, paths: Arc<RwLock<HashMap<TourId, PathBuf>>>) -> Self {
        AsyncTourFileManager { tours, paths }
    }

    pub fn start(&self) {
        // TODO: Auto-save loop
    }
}

impl TourFileManager for AsyncTourFileManager {
    fn save_tour(&self, tour_id: TourId) -> Result<()> {
        let tours = self.tours.read().unwrap();
        let paths = self.paths.read().unwrap();
        let tour = tours.get(&tour_id);
        let path = paths.get(&tour_id);
        if let (Some(tour), Some(path)) = (tour, path) {
            let tour_source = serialize::serialize_tour(tour.clone())
                .context(ErrorKind::FailedToSerializeTour)?;
            fs::write(path, tour_source).context(ErrorKind::FailedToWriteTour)?;
            Ok(())
        } else if tour.is_none() {
            Err(ErrorKind::NoTourWithID.attach("ID", tour_id))
        } else {
            Err(ErrorKind::NoPathForTour.attach("ID", tour_id))
        }
    }

    fn load_tour(&self, path: PathBuf) -> Result<Tour> {
        let tour_source = fs::read_to_string(path).context(ErrorKind::FailedToReadTour)?;
        let tour = serialize::parse_tour(&tour_source).context(ErrorKind::FailedToParseTour)?;
        Ok(tour)
    }

    fn delete_tour(&self, tour_id: TourId) -> Result<()> {
        let mut tours = self.tours.write().unwrap();
        let mut paths = self.paths.write().unwrap();

        if !tours.contains_key(&tour_id) {
            return Err(ErrorKind::NoTourWithID.attach("ID", tour_id));
        }

        tours.remove(&tour_id);
        paths.remove(&tour_id);
        Ok(())
    }

    fn reload_tour(&self, tour_id: TourId) -> Result<Tour> {
        let paths = self.paths.read().unwrap();
        let path = paths
            .get(&tour_id)
            .ok_or_else(|| ErrorKind::NoPathForTour.attach("TourId", tour_id.clone()))?;
        self.load_tour(path.to_path_buf())
    }

    fn set_tour_path(&self, tour_id: TourId, path: PathBuf) {
        let mut paths = self.paths.write().unwrap();
        paths.insert(tour_id, path);
    }
}
