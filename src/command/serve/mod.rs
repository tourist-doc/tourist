use crate::types::path::{AbsolutePathBuf, RelativePathBuf};
use crate::types::{Index, Stop, StopReference, Tour};
use crate::vcs::VCS;
use failure::ResultExt;
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

fn resolve_path<I: Index>(
    index: &I,
    repository: &str,
    rel_path: &RelativePathBuf,
) -> Result<AbsolutePathBuf> {
    let abs = index
        .get(repository)
        .ok_or(ErrorKind::RepositoryNotInIndex {
            repo: repository.to_owned(),
        })?;
    Ok(abs.join_rel(rel_path))
}

fn find_path_in_context<I: Index>(
    index: &I,
    path: PathBuf,
) -> Result<(RelativePathBuf, String, AbsolutePathBuf)> {
    let deep =
        AbsolutePathBuf::new(path.clone()).ok_or_else(|| ErrorKind::ExpectedAbsolutePath {
            path: format!("{}", path.display()),
        })?;
    for (repo_name, repo_path) in index.all() {
        if let Some(rel) = deep.try_relative(repo_path.as_absolute_path()) {
            return Ok((rel, repo_name.to_owned(), repo_path.clone()));
        }
    }
    Err(ErrorKind::PathNotInIndex {
        path: format!("{}", path.display()),
    }
    .into())
}

type Tracker = HashMap<TourId, Tour>;

struct Tourist<M: TourFileManager, V: VCS, I: Index> {
    tours: Arc<RwLock<Tracker>>,
    edits: Arc<RwLock<HashSet<TourId>>>,
    manager: M,
    vcs: V,
    index: I,
}

impl<M: TourFileManager, V: VCS, I: Index> Tourist<M, V, I> {
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

    fn with_tour<T, F>(&self, tour_id: &str, f: F) -> Result<T>
    where
        F: FnOnce(&Tour) -> Result<T>,
    {
        let tours = self.get_tours();
        let tour = tours.get(tour_id).ok_or(ErrorKind::NoTourFound {
            id: tour_id.to_owned(),
        })?;
        f(tour)
    }

    fn with_tour_mut<T, F>(&self, tour_id: &str, f: F) -> Result<T>
    where
        F: FnOnce(&mut Tour) -> Result<T>,
    {
        let mut tours = self.get_tours_mut();
        let tour = tours.get_mut(tour_id).ok_or(ErrorKind::NoTourFound {
            id: tour_id.to_owned(),
        })?;
        f(tour)
    }

    fn with_stop<T, F>(&self, tour_id: &str, stop_id: &str, f: F) -> Result<T>
    where
        F: FnOnce(&Stop) -> Result<T>,
    {
        self.with_tour(tour_id, |tour| {
            let stop =
                tour.stops
                    .iter()
                    .find(|s| s.id == *stop_id)
                    .ok_or(ErrorKind::NoStopFound {
                        tour_id: tour_id.to_owned(),
                        stop_id: stop_id.to_owned(),
                    })?;
            f(stop)
        })
    }

    fn with_stop_mut<T, F>(&self, tour_id: &str, stop_id: &str, f: F) -> Result<T>
    where
        F: FnOnce(&mut Stop) -> Result<T>,
    {
        self.with_tour_mut(tour_id, |tour| {
            let stop =
                tour.stops
                    .iter_mut()
                    .find(|s| s.id == *stop_id)
                    .ok_or(ErrorKind::NoStopFound {
                        tour_id: tour_id.to_owned(),
                        stop_id: stop_id.to_owned(),
                    })?;
            f(stop)
        })
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

impl<M: TourFileManager, V: VCS, I: Index> TouristRpc for Tourist<M, V, I> {
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
        self.with_tour(&tour_id, |tour| {
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
        })
        .as_json_result()
    }

    fn edit_tour_metadata(&self, tour_id: TourId, delta: TourMetadata) -> JsonResult<()> {
        self.with_tour_mut(&tour_id, |tour| {
            delta.apply_to(tour);
            Ok(())
        })
        .as_json_result()
    }

    fn refresh_tour(&self, tour_id: TourId, commit: Option<String>) -> JsonResult<()> {
        self.with_tour_mut(&tour_id, |tour| {
            let mut new_versions = HashMap::new();
            for mut stop in tour.stops.iter_mut() {
                let repo_path = resolve_path(&self.index, &stop.repository, &stop.path)?;
                let tour_version = tour.repositories.get(&stop.repository).ok_or(
                    ErrorKind::NoVersionForRepository {
                        repo: stop.repository.clone(),
                    },
                )?;
                let target_version = if let Some(commit) = commit.clone() {
                    commit
                } else {
                    self.vcs
                        .get_current_version(repo_path.as_absolute_path())
                        .context(ErrorKind::DiffFailed)?
                };
                let changes = self
                    .vcs
                    .diff_with_version(repo_path.as_absolute_path(), &tour_version, &target_version)
                    .context(ErrorKind::DiffFailed)?;
                if let Some(file_changes) = changes.for_file(&stop.path) {
                    stop.line = file_changes.adjust_line(stop.line).unwrap();
                }
                new_versions.insert(stop.repository.clone(), target_version.clone());
            }
            tour.repositories.extend(new_versions);
            Ok(())
        })
        .as_json_result()
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
        let (rel_path, repo, repo_path) =
            find_path_in_context(&self.index, path).as_json_result()?;
        let stop = Stop {
            id: id.clone(),
            title,
            description: "".to_owned(),
            path: rel_path,
            repository: repo.clone(),
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
        tour.repositories.insert(
            repo,
            self.vcs
                .get_current_version(repo_path.as_absolute_path())
                .context(ErrorKind::DiffFailed)
                .as_json_result()?,
        );
        Ok(id)
    }

    fn view_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<StopView> {
        self.with_stop(&tour_id, &stop_id, |stop| {
            Ok(StopView {
                title: stop.title.clone(),
                description: stop.description.clone(),
                repository: stop.repository.clone(),
                children: stop
                    .children
                    .iter()
                    .map(|sr| self.view_stop_reference(sr))
                    .collect::<Result<Vec<_>>>()?,
            })
        })
        .as_json_result()
    }

    fn edit_stop_metadata(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        delta: StopMetadata,
    ) -> JsonResult<()> {
        self.with_stop_mut(&tour_id, &stop_id, |stop| {
            delta.apply_to(stop);
            Ok(())
        })
        .as_json_result()
    }

    fn link_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> JsonResult<()> {
        self.with_stop_mut(&tour_id, &stop_id, |stop| {
            stop.children.push(StopReference {
                tour_id: other_tour_id,
                stop_id: other_stop_id,
            });
            Ok(())
        })
        .as_json_result()
    }

    fn locate_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        naive: bool,
    ) -> JsonResult<Option<(PathBuf, usize)>> {
        self.with_tour(&tour_id, |tour| {
            self.with_stop(&tour_id, &stop_id, |stop| {
                let version = tour.repositories.get(&stop.repository).ok_or(
                    ErrorKind::NoVersionForRepository {
                        repo: stop.repository.clone(),
                    },
                )?;
                let path = resolve_path(&self.index, &stop.repository, &stop.path)?;
                let line = if naive {
                    Some(stop.line)
                } else {
                    let changes = self
                        .vcs
                        .diff_with_worktree(path.as_absolute_path(), version)
                        .context(ErrorKind::DiffFailed)?;
                    if let Some(changes) = changes.for_file(&stop.path) {
                        changes.adjust_line(stop.line)
                    } else {
                        Some(stop.line)
                    }
                };
                Ok(if let Some(line) = line {
                    Some((path.as_path_buf().clone(), line))
                } else {
                    None
                })
            })
        })
        .as_json_result()
    }

    fn remove_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<()> {
        self.with_tour_mut(&tour_id, |tour| {
            let n = tour.stops.len();
            tour.stops.retain(|stop| stop.id != stop_id);
            if n == tour.stops.len() {
                // No change in length means that the stop was not deleted successfully
                Err(ErrorKind::NoStopFound {
                    tour_id: tour_id.clone(),
                    stop_id,
                }
                .into())
            } else {
                Ok(())
            }
        })
        .as_json_result()
    }

    fn index_repository(&self, repo_name: String, path: Option<PathBuf>) -> JsonResult<()> {
        if let Some(path) = path {
            let abs_path = AbsolutePathBuf::new(path.clone())
                .ok_or(ErrorKind::ExpectedAbsolutePath {
                    path: format!("{}", path.display()),
                })
                .as_json_result()?;
            self.index.set(&repo_name, &abs_path);
        } else {
            self.index.unset(&repo_name);
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

pub struct Serve<V: VCS, I: Index> {
    vcs: V,
    index: I,
}

impl<V: VCS, I: Index> Serve<V, I> {
    pub fn new(vcs: V, index: I) -> Self {
        Serve { vcs, index }
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
                vcs: self.vcs.clone(),
                index: self.index.clone(),
                edits: Arc::new(RwLock::new(HashSet::new())),
            }
            .to_delegate(),
        );
        ServerBuilder::new(io).build();
    }
}
