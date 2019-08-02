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

#[cfg(test)]
pub mod mock;

pub trait TourFileManager: Send + Sync + 'static {
    fn save_tour(&self, tour_id: TourId) -> Result<()>;
    fn load_tour(&self, path: PathBuf) -> Result<Tour>;
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
        unimplemented!();
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
}
