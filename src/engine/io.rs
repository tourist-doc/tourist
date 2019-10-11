use super::TourId;
use crate::error::{ErrorKind, Result};
use crate::serialize;
use crate::types::Tour;
use failure::ResultExt;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub trait TourFileManager {
    fn save_tour(&self, tour: &Tour) -> Result<()>;
    fn load_tour(&self, path: PathBuf) -> Result<Tour>;
    fn delete_tour(&mut self, tour_id: TourId) -> Result<()>;
    fn set_tour_path(&mut self, tour_id: TourId, path: PathBuf);
    fn reload_tour(&self, tour_id: TourId) -> Result<Tour>;
}

pub struct BasicTourFileManager {
    paths: HashMap<TourId, PathBuf>,
}

impl BasicTourFileManager {
    pub fn new(paths: HashMap<TourId, PathBuf>) -> Self {
        BasicTourFileManager { paths }
    }
}

impl TourFileManager for BasicTourFileManager {
    fn save_tour(&self, tour: &Tour) -> Result<()> {
        let path = self.paths.get(&tour.id);
        if let Some(path) = path {
            let tour_source = serialize::serialize_tour(tour.clone())
                .context(ErrorKind::FailedToSerializeTour)?;
            fs::write(path, tour_source).context(ErrorKind::FailedToWriteTour)?;
            Ok(())
        } else {
            Err(ErrorKind::NoPathForTour.attach("ID", tour.id.clone()))
        }
    }

    fn load_tour(&self, path: PathBuf) -> Result<Tour> {
        let tour_source = fs::read_to_string(path).context(ErrorKind::FailedToReadTour)?;
        let tour = serialize::parse_tour(&tour_source).context(ErrorKind::FailedToParseTour)?;
        Ok(tour)
    }

    fn delete_tour(&mut self, tour_id: TourId) -> Result<()> {
        self.paths.remove(&tour_id);
        Ok(())
    }

    fn reload_tour(&self, tour_id: TourId) -> Result<Tour> {
        let path = self
            .paths
            .get(&tour_id)
            .ok_or_else(|| ErrorKind::NoPathForTour.attach("TourId", tour_id.clone()))?;
        self.load_tour(path.to_path_buf())
    }

    fn set_tour_path(&mut self, tour_id: TourId, path: PathBuf) {
        self.paths.insert(tour_id, path);
    }
}
