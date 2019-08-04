use crate::types::path::{AbsolutePathBuf, RelativePathBuf};
use crate::types::{Index, Stop, StopReference, Tour};
use jsonrpc_core;
use jsonrpc_core::Result as JsonResult;
use jsonrpc_stdio_server::ServerBuilder;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;

#[cfg(test)]
mod tests;

mod error;
mod interface;
mod io;
use error::{AsJsonResult, ErrorKind, Result};
pub use interface::*;
use io::{AsyncTourFileManager, TourFileManager};

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

fn resolve_path(
    _index: &Index,
    _repository: &str,
    _rel_path: &RelativePathBuf,
) -> Result<AbsolutePathBuf> {
    unimplemented!();
}

fn find_path_in_context(index: &Index, path: PathBuf) -> Result<(RelativePathBuf, String)> {
    let deep =
        AbsolutePathBuf::new(path.clone()).ok_or_else(|| ErrorKind::ExpectedAbsolutePath {
            path: format!("{}", path.display()),
        })?;
    for (repo_name, repo_path) in index.iter() {
        if let Some(rel) = deep.try_relative(repo_path.as_absolute_path()) {
            return Ok((rel, repo_name.to_owned()));
        }
    }
    Err(ErrorKind::PathNotInIndex {
        path: format!("{}", path.display()),
    }
    .into())
}

type Tracker = HashMap<TourId, Tour>;

struct Tourist<M: TourFileManager> {
    index: Arc<RwLock<Index>>,
    tours: Arc<RwLock<Tracker>>,
    edits: Arc<RwLock<HashSet<TourId>>>,
    manager: M,
}

impl<M: TourFileManager> Tourist<M> {
    /// Get a reference to the currently managed map of tours. In the event of a `PoisonError`, we
    /// panic.
    fn get_tours(&self) -> RwLockReadGuard<HashMap<TourId, Tour>> {
        self.tours.read().unwrap()
    }

    /// Get a mutable reference to the currently managed map of tours. In the event of a
    /// `PoisonError`, we panic.
    fn get_tours_mut(&self) -> RwLockWriteGuard<HashMap<TourId, Tour>> {
        self.tours.write().unwrap()
    }

    /// Get a reference to the current set of editable tours. In the event of a `PoisonError`, we
    /// panic.
    fn get_edits(&self) -> RwLockReadGuard<HashSet<TourId>> {
        self.edits.read().unwrap()
    }

    /// Get a mutable reference to the current set of editable tours. In the event of a
    /// `PoisonError`, we panic.
    fn get_edits_mut(&self) -> RwLockWriteGuard<HashSet<TourId>> {
        self.edits.write().unwrap()
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

    fn view_stop_reference(&self, sr: &StopReference) -> Result<StopReferenceView> {
        let tours = self.get_tours();
        let other_tour = tours.get(&sr.tour_id);
        if let Some(other_tour) = other_tour {
            let (other_stop_id, other_stop_title) = if let Some(other_stop_id) = &sr.stop_id {
                let other_stop = other_tour
                    .stops
                    .iter()
                    .find(|s| s.id == *other_stop_id)
                    .ok_or(ErrorKind::NoStopFound {
                        tour_id: sr.tour_id.clone(),
                        stop_id: other_stop_id.clone(),
                    })?;
                (Some(other_stop_id.clone()), Some(other_stop.title.clone()))
            } else {
                (None, None)
            };
            Ok(StopReferenceView::Tracked {
                tour_id: sr.tour_id.clone(),
                tour_title: other_tour.title.clone(),
                stop_id: other_stop_id.clone(),
                stop_title: other_stop_title.clone(),
            })
        } else {
            Ok(StopReferenceView::Untracked {
                tour_id: sr.tour_id.clone(),
                stop_id: sr.stop_id.clone(),
            })
        }
    }
}

impl<M: TourFileManager> TouristRpc for Tourist<M> {
    fn list_tours(&self) -> JsonResult<Vec<(TourId, String)>> {
        let tours = self.get_tours();
        Ok(tours
            .values()
            .map(|tour| (tour.id.clone(), tour.title.clone()))
            .collect())
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
        let tour = self.manager.load_tour(path).as_json_result()?;
        let mut tours = self.get_tours_mut();
        let id = tour.id.clone();
        tours.insert(tour.id.clone(), tour);
        Ok(id)
    }

    fn set_tour_edit(&self, tour_id: TourId, edit: bool) -> JsonResult<()> {
        let mut edits = self.get_edits_mut();
        if edit {
            edits.insert(tour_id);
        } else {
            edits.remove(&tour_id);
        }
        Ok(())
    }

    fn view_tour(&self, tour_id: TourId) -> JsonResult<TourView> {
        let tours = self.get_tours();
        let tour = tours
            .get(&tour_id)
            .ok_or(ErrorKind::NoTourFound {
                id: tour_id.clone(),
            })
            .as_json_result()?;
        let edits = self.get_edits();
        Ok(TourView {
            title: tour.title.clone(),
            description: tour.description.clone(),
            stops: tour
                .stops
                .iter()
                .map(|stop| (stop.id.clone(), stop.title.clone()))
                .collect(),
            repositories: tour
                .repositories
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            edit: edits.contains(&tour_id),
        })
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
        path: PathBuf,
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

    fn view_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<StopView> {
        let tours = self.get_tours();
        let tour = tours
            .get(&tour_id)
            .ok_or(ErrorKind::NoTourFound {
                id: tour_id.clone(),
            })
            .as_json_result()?;
        let stop = tour
            .stops
            .iter()
            .find(|s| s.id == stop_id)
            .ok_or(ErrorKind::NoStopFound { tour_id, stop_id })
            .as_json_result()?;
        Ok(StopView {
            title: stop.title.clone(),
            description: stop.description.clone(),
            repository: stop.repository.clone(),
            children: stop
                .children
                .iter()
                .map(|sr| self.view_stop_reference(sr))
                .collect::<Result<Vec<_>>>()
                .as_json_result()?,
        })
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

    fn link_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
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
        stop.children.push(StopReference {
            tour_id: other_tour_id,
            stop_id: other_stop_id,
        });
        Ok(())
    }

    fn locate_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        naive: bool,
    ) -> JsonResult<Option<(PathBuf, usize)>> {
        let tours = self.get_tours();
        let tour = tours
            .get(&tour_id)
            .ok_or(ErrorKind::NoTourFound {
                id: tour_id.clone(),
            })
            .as_json_result()?;
        let stop = tour
            .stops
            .iter()
            .find(|s| s.id == stop_id)
            .ok_or(ErrorKind::NoStopFound { tour_id, stop_id })
            .as_json_result()?;
        let path =
            resolve_path(&self.get_index(), &stop.repository, &stop.path).as_json_result()?;
        let line = if naive { stop.line } else { unimplemented!() };
        Ok(Some((path.as_path_buf().clone(), line)))
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
        for tour in self.get_tours().values() {
            self.manager.save_tour(tour.id.clone()).as_json_result()?;
        }
        Ok(())
    }

    fn save_tour(&self, tour_id: TourId, path: Option<PathBuf>) -> JsonResult<()> {
        if let Some(path) = path {
            self.manager.set_tour_path(tour_id.clone(), path);
        }
        self.manager.save_tour(tour_id).as_json_result()?;
        Ok(())
    }

    fn delete_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.forget_tour(tour_id.clone())?;
        self.manager.delete_tour(tour_id).as_json_result()?;
        Ok(())
    }
}

pub struct Serve;

impl Serve {
    pub fn new() -> Self {
        Serve
    }

    pub fn process(&self) {
        let mut io = jsonrpc_core::IoHandler::new();
        let tours = Arc::new(RwLock::new(HashMap::new()));
        let manager = AsyncTourFileManager::new(Arc::clone(&tours));
        manager.start();
        io.extend_with(
            Tourist {
                tours,
                manager,
                edits: Arc::new(RwLock::new(HashSet::new())),
                index: Arc::new(RwLock::new(HashMap::new())),
            }
            .to_delegate(),
        );
        ServerBuilder::new(io).build();
    }
}
