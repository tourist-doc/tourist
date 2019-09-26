use crate::error::{AsJsonResult, ErrorKind, Result};
use crate::index::Index;
use crate::types::path::{AbsolutePathBuf, RelativePathBuf};
use crate::types::{Stop, StopReference, Tour};
use crate::vcs::VCS;
use failure::ResultExt;
use jsonrpc_core;
use jsonrpc_core::Result as JsonResult;
use jsonrpc_stdio_server::ServerBuilder;
use slog_scope::info;
use std::cmp;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;

#[cfg(test)]
mod tests;

mod interface;
mod io;
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
        .get(repository)?
        .ok_or_else(|| ErrorKind::RepositoryNotInIndex.attach("Repository", repository))?;
    Ok(abs.join_rel(rel_path))
}

fn find_path_in_context<I: Index>(
    index: &I,
    path: PathBuf,
) -> Result<(RelativePathBuf, String, AbsolutePathBuf)> {
    let deep = AbsolutePathBuf::new(path.clone())
        .ok_or_else(|| ErrorKind::ExpectedAbsolutePath.attach("Path", path.display()))?;
    for (repo_name, repo_path) in index.all()? {
        if let Some(rel) = deep.try_relative(repo_path.as_absolute_path()) {
            return Ok((rel, repo_name.to_owned(), repo_path.clone()));
        }
    }
    Err(ErrorKind::PathNotInIndex.attach("Path", path.display()))
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
        let tour = tours
            .get(tour_id)
            .ok_or_else(|| ErrorKind::NoTourFound.attach("ID", tour_id))?;
        f(tour)
    }

    fn with_tour_mut<T, F>(&self, tour_id: &str, f: F) -> Result<T>
    where
        F: FnOnce(&mut Tour) -> Result<T>,
    {
        let mut tours = self.get_tours_mut();
        let tour = tours
            .get_mut(tour_id)
            .ok_or_else(|| ErrorKind::NoTourFound.attach("ID", tour_id))?;
        f(tour)
    }

    fn with_stop<T, F>(&self, tour_id: &str, stop_id: &str, f: F) -> Result<T>
    where
        F: FnOnce(&Stop) -> Result<T>,
    {
        self.with_tour(tour_id, |tour| {
            let stop = tour
                .stops
                .iter()
                .find(|s| s.id == *stop_id)
                .ok_or_else(|| {
                    ErrorKind::NoStopFound
                        .attach("Tour ID", tour_id)
                        .attach("Stop ID", stop_id)
                })?;
            f(stop)
        })
    }

    fn with_stop_mut<T, F>(&self, tour_id: &str, stop_id: &str, f: F) -> Result<T>
    where
        F: FnOnce(&mut Stop) -> Result<T>,
    {
        self.with_tour_mut(tour_id, |tour| {
            let stop = tour
                .stops
                .iter_mut()
                .find(|s| s.id == *stop_id)
                .ok_or_else(|| {
                    ErrorKind::NoStopFound
                        .attach("Tour ID", tour_id)
                        .attach("Stop ID", stop_id)
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
                    .ok_or_else(|| {
                        ErrorKind::NoStopFound
                            .attach("Tour ID", sr.tour_id.clone())
                            .attach("Stop ID", other_stop_id)
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
                let tour_version = tour.repositories.get(&stop.repository).ok_or_else(|| {
                    ErrorKind::NoVersionForRepository.attach("Repository", stop.repository.clone())
                })?;
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
                    match file_changes.adjust_line(stop.line) {
                        Some(line) => {
                            stop.line = line;
                        }
                        None => {
                            stop.broken = Some("line was deleted".to_owned());
                        }
                    }
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
            return Err(ErrorKind::NoTourFound.attach("ID", tour_id)).as_json_result();
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
            broken: None,
        };
        let mut tours = self.get_tours_mut();
        let tour = tours
            .get_mut(&tour_id)
            .ok_or_else(|| ErrorKind::NoTourFound.attach("ID", tour_id))
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

    fn move_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        path: PathBuf,
        line: usize,
    ) -> JsonResult<()> {
        let (rel_path, repo, repo_path) =
            find_path_in_context(&self.index, path).as_json_result()?;
        // Two things need to happen here:
        // 1. The stop needs to be moved to the approapriate relative stop/line.
        // 2. If this change happens to modify `tour.repositories`, that needs to be handled.
        // Unfortunately, both of these operations could fail -- the stop might not exist, and the
        // new file might not be in a git repository. We wouldn't want to make one mutation, then
        // crash, and not make the other. The solution is to:
        self.with_tour_mut(&tour_id, |tour| {
            // First, make sure the stop actually exists in the tour
            tour.stops.iter().find(|s| s.id == stop_id).ok_or_else(|| {
                ErrorKind::NoStopFound
                    .attach("Tour ID", &tour_id)
                    .attach("Stop ID", &stop_id)
            })?;
            // Then, make the change to tour.repositories
            tour.repositories.insert(
                repo,
                self.vcs
                    .get_current_version(repo_path.as_absolute_path())
                    .context(ErrorKind::DiffFailed)?,
            );
            Ok(())
        })
        .as_json_result()?;
        // Finally, once we're sure that no more failure can occur, make the change to the stop
        self.with_stop_mut(&tour_id, &stop_id, |stop| {
            stop.path = rel_path;
            stop.line = line;
            stop.broken = None;
            Ok(())
        })
        .as_json_result()
    }

    fn reorder_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        position_delta: isize,
    ) -> JsonResult<()> {
        // Clamp `val` to be within `min` and `max` inclusive. Assumes `min <= max`
        fn clamp(val: isize, min: isize, max: isize) -> isize {
            cmp::min(cmp::max(val, min), max)
        }

        self.with_tour_mut(&tour_id, |tour| {
            let idx = tour
                .stops
                .iter()
                .position(|stop| stop.id == stop_id)
                .ok_or_else(|| {
                    ErrorKind::NoStopFound
                        .attach("Tour ID", &tour_id)
                        .attach("Stop ID", stop_id)
                })? as isize;
            let end_of_list = (tour.stops.len() - 1) as isize;
            tour.stops.swap(
                usize::try_from(idx).context(ErrorKind::UnknownFailure)?,
                usize::try_from(clamp(idx + position_delta, 0, end_of_list))
                    .context(ErrorKind::UnknownFailure)?,
            );
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

    fn unlink_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> JsonResult<()> {
        self.with_stop_mut(&tour_id, &stop_id, |stop| {
            stop.children
                .retain(|r| !(r.tour_id == other_tour_id && r.stop_id == other_stop_id));
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
                if stop.broken.is_some() {
                    // broken stop, can't locate
                    return Ok(None);
                }
                let path = resolve_path(&self.index, &stop.repository, &stop.path)?;
                let line = if naive {
                    Some(stop.line)
                } else {
                    let version = tour.repositories.get(&stop.repository).ok_or_else(|| {
                        ErrorKind::NoVersionForRepository
                            .attach("Repository", stop.repository.clone())
                    })?;
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
                Ok(line.map(|l| (path.as_path_buf().clone(), l)))
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
                Err(ErrorKind::NoStopFound
                    .attach("Tour ID", tour_id.clone())
                    .attach("Stop ID", stop_id.clone()))
            } else {
                Ok(())
            }
        })
        .as_json_result()
    }

    fn index_repository(&self, repo_name: String, path: Option<PathBuf>) -> JsonResult<()> {
        if let Some(path) = path {
            let abs_path = AbsolutePathBuf::new(path.clone())
                .ok_or_else(|| ErrorKind::ExpectedAbsolutePath.attach("Path", path.display()))
                .as_json_result()?;
            self.index.set(&repo_name, &abs_path).as_json_result()
        } else {
            self.index.unset(&repo_name).as_json_result()
        }
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

    pub fn process(&self, init_tours: Vec<Tour>) {
        info!("running server with initial tours {:?}", init_tours);
        let mut io = jsonrpc_core::IoHandler::new();
        let tours = Arc::new(RwLock::new(
            init_tours
                .into_iter()
                .map(|tour| (tour.id.clone(), tour))
                .collect(),
        ));
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
        info!("starting tourist server");
        ServerBuilder::new(io).build();
    }
}
