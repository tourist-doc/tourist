use crate::serialize::parse_tour;
use crate::types::path::{AbsolutePathBuf, RelativePathBuf};
use crate::types::{Index, Stop, Tour};
use failure::{Fail, ResultExt};
use jsonrpc_core;
use jsonrpc_core::Result as JsonResult;
use jsonrpc_stdio_server::ServerBuilder;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;

mod error;
mod interface;
use error::{AsJsonResult, ErrorKind};
pub use interface::*;

impl StopMetadata {
    fn apply_to(mut self, stop: &mut Stop) {
        if let Some(title) = self.title.take() {
            stop.title = title;
        }
        if let Some(description) = self.description.take() {
            stop.description = description;
        }
    }
}

impl TourMetadata {
    fn apply_to(mut self, tour: &mut Tour) {
        if let Some(title) = self.title.take() {
            tour.title = title;
        }
        if let Some(description) = self.description.take() {
            tour.description = description;
        }
    }
}

fn find_path_in_context(
    index: &Index,
    path: String,
) -> Result<(RelativePathBuf, String), impl Fail> {
    let deep = AbsolutePathBuf::new(PathBuf::from(path.clone()))
        .ok_or_else(|| ErrorKind::ExpectedAbsolutePath { path: path.clone() })?;
    for (repo_name, repo_path) in index.iter() {
        if let Some(rel) = deep.try_relative(repo_path.as_absolute_path()) {
            return Ok((rel, repo_name.to_owned()));
        }
    }
    Err(ErrorKind::PathNotInIndex { path })
}

type Tracker = HashMap<TourId, Tour>;

struct Tourist {
    index: Arc<RwLock<Index>>,
    tours: Arc<RwLock<Tracker>>,
}

impl Default for Tourist {
    fn default() -> Tourist {
        Tourist {
            index: Arc::new(RwLock::new(HashMap::new())),
            tours: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Tourist {
    /// Get a reference to the currently managed map of tours. In the event of a `PoisonError`, we
    /// panic.
    #[allow(dead_code)]
    fn get_tours(&self) -> RwLockReadGuard<HashMap<TourId, Tour>> {
        self.tours.read().unwrap()
    }

    /// Get a mutable reference to the currently managed map of tours. In the event of a
    /// `PoisonError`, we panic.
    fn get_tours_mut(&self) -> RwLockWriteGuard<HashMap<TourId, Tour>> {
        self.tours.write().unwrap()
    }

    /// Get a reference to the index of git repositories. In the event of a `PoisonError`, we
    /// panic.
    fn get_index(&self) -> RwLockReadGuard<Index> {
        self.index.read().unwrap()
    }

    /// Get a mutable reference to the index of git repositories. In the event of a `PoisonError`,
    /// we panic.
    fn get_index_mut(&self) -> RwLockWriteGuard<Index> {
        self.index.write().unwrap()
    }
}

impl TouristRpc for Tourist {
    fn list_tours(&self) -> JsonResult<Vec<(TourId, String)>> {
        unimplemented!();
    }

    fn create_tour(&self, title: String) -> JsonResult<TourId> {
        let id = format!("{}", Uuid::new_v4().to_simple());
        let new_tour = Tour {
            protocol_version: "1.0".to_owned(),
            id: id.clone(),
            title,
            description: "".to_owned(),
            stops: Vec::new(),
            repositories: HashMap::new(),
            generator: 0,
        };
        self.get_tours_mut().insert(id.clone(), new_tour);
        Ok(id)
    }

    fn open_tour(&self, path: PathBuf, _edit: bool) -> JsonResult<TourId> {
        let tour_source = fs::read_to_string(path)
            .context(ErrorKind::FailedToReadTour)
            .as_json_result()?;
        let tour = parse_tour(&tour_source)
            .context(ErrorKind::FailedToParseTour)
            .as_json_result()?;
        let mut tours = self.get_tours_mut();
        let id = tour.id.clone();
        tours.insert(tour.id.clone(), tour);
        Ok(id)
    }

    fn set_tour_edit(&self, _tour_id: TourId, _edit: bool) -> JsonResult<()> {
        unimplemented!();
    }

    fn view_tour(&self, _tour_id: TourId) -> JsonResult<TourView> {
        unimplemented!();
    }

    fn edit_tour_metadata(&self, tour_id: TourId, delta: TourMetadata) -> JsonResult<()> {
        let mut tours = self.get_tours_mut();
        let tour = tours
            .get_mut(&tour_id)
            .ok_or(ErrorKind::NoTourFound { id: tour_id })
            .as_json_result()?;
        delta.apply_to(tour);
        Ok(())
    }

    fn refresh_tour(&self, _tour_id: TourId, _commit: Option<String>) -> JsonResult<()> {
        unimplemented!();
    }

    fn forget_tour(&self, tour_id: TourId) -> JsonResult<()> {
        let mut tours = self.get_tours_mut();
        if !tours.contains_key(&tour_id) {
            return Err(ErrorKind::NoTourFound { id: tour_id }).as_json_result();
        }
        tours.remove(&tour_id);
        Ok(())
    }

    fn create_stop(
        &self,
        tour_id: TourId,
        title: String,
        path: String,
        line: usize,
    ) -> JsonResult<StopId> {
        let id = format!("{}", Uuid::new_v4().to_simple());
        let (rel_path, repo) = find_path_in_context(&self.get_index(), path).as_json_result()?;
        let stop = Stop {
            id: id.clone(),
            title,
            description: "".to_owned(),
            path: rel_path,
            repository: repo,
            line,
            children: Vec::new(),
        };
        let mut tours = self.get_tours_mut();
        let tour = tours
            .get_mut(&tour_id)
            .ok_or(ErrorKind::NoTourFound {
                id: tour_id.clone(),
            })
            .as_json_result()?;
        tour.stops.push(stop);
        Ok(id)
    }

    fn view_stop(&self, _tour_id: TourId, _stop_id: StopId) -> JsonResult<StopView> {
        unimplemented!();
    }

    fn edit_stop_metadata(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        delta: StopMetadata,
    ) -> JsonResult<()> {
        let mut tours = self.get_tours_mut();
        let tour = tours
            .get_mut(&tour_id)
            .ok_or(ErrorKind::NoTourFound {
                id: tour_id.clone(),
            })
            .as_json_result()?;
        let stop = tour
            .stops
            .iter_mut()
            .find(|s| s.id == stop_id)
            .ok_or(ErrorKind::NoStopFound { tour_id, stop_id })
            .as_json_result()?;
        delta.apply_to(stop);
        Ok(())
    }

    fn locate_stop(
        &self,
        _tour_id: TourId,
        _stop_id: StopId,
        _naive: bool,
    ) -> JsonResult<Option<(PathBuf, usize)>> {
        unimplemented!();
    }

    fn remove_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<()> {
        let mut tours = self.get_tours_mut();
        let tour = tours
            .get_mut(&tour_id)
            .ok_or(ErrorKind::NoTourFound {
                id: tour_id.clone(),
            })
            .as_json_result()?;
        let n = tour.stops.len();
        tour.stops.retain(|stop| stop.id != stop_id);
        if n == tour.stops.len() {
            // No change in length means that the stop was not deleted successfully
            Err(ErrorKind::NoStopFound { tour_id, stop_id }).as_json_result()
        } else {
            Ok(())
        }
    }

    fn index_repository(&self, repo_name: String, path: Option<PathBuf>) -> JsonResult<()> {
        let mut index = self.get_index_mut();
        if let Some(path) = path {
            let abs_path = AbsolutePathBuf::new(path.clone())
                .ok_or(ErrorKind::ExpectedAbsolutePath {
                    path: format!("{}", path.display()),
                })
                .as_json_result()?;
            index.insert(repo_name, abs_path);
        } else {
            index.remove(&repo_name);
        }
        Ok(())
    }

    fn save_all(&self) -> JsonResult<()> {
        unimplemented!();
    }

    fn save_tour(&self, _tour_id: TourId, _path: Option<PathBuf>) -> JsonResult<()> {
        unimplemented!();
    }

    fn delete_tour(&self, _tour_id: TourId) -> JsonResult<()> {
        unimplemented!();
    }
}

pub struct Serve;

impl Serve {
    pub fn new() -> Self {
        Serve
    }

    pub fn process(&self) {
        let mut io = jsonrpc_core::IoHandler::new();
        io.extend_with(Tourist::default().to_delegate());
        ServerBuilder::new(io).build();
    }
}

#[cfg(test)]
mod tests {
    use super::{Tourist, TouristRpc};
    use std::fs;
    use tempdir::TempDir;

    #[test]
    fn create_tour_test() {
        let tourist = Tourist::default();
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
        let tour_file = TempDir::new("tours").unwrap().into_path().join("test.tour");
        fs::write(&tour_file, include_str!("../../../data/empty-tour.tour"))
            .expect("Failed to write to file");

        let tourist = Tourist::default();
        tourist
            .open_tour(tour_file, false)
            .expect("Call to open failed");
        let tours = tourist.tours.read().expect("Lock was poisoned");
        let tour = tours.values().next().expect("No tours found");
        assert_eq!(tour.title, "My first tour");
        assert_eq!(tour.stops, vec![]);
    }
}
