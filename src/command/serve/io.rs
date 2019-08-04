use super::error::{ErrorKind, Result};
use super::interface::TourId;
use super::Tracker;
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
}

pub struct AsyncTourFileManager {
    tours: Arc<RwLock<Tracker>>,
    paths: Arc<RwLock<HashMap<TourId, PathBuf>>>,
}

impl AsyncTourFileManager {
    pub fn new(tours: Arc<RwLock<Tracker>>) -> Self {
        AsyncTourFileManager {
            tours,
            paths: Arc::new(RwLock::new(HashMap::new())),
        }
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
            Err(ErrorKind::NoTourFound { id: tour_id }.into())
        } else {
            Err(ErrorKind::NoPathForTour { id: tour_id }.into())
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
            return Err(ErrorKind::NoTourFound { id: tour_id }.into());
        }

        tours.remove(&tour_id);
        paths.remove(&tour_id);
        Ok(())
    }

    fn set_tour_path(&self, tour_id: TourId, path: PathBuf) {
        let mut paths = self.paths.write().unwrap();
        paths.insert(tour_id, path);
    }
}