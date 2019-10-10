use crate::error::{AsJsonResult, Error, ErrorKind, Result};
use crate::index::Index;
use crate::types::path::{AbsolutePathBuf, RelativePathBuf};
use crate::types::{Stop, StopReference, Tour};
use crate::vcs::VCS;
use failure::ResultExt;
use jsonrpc_core;
use jsonrpc_core::Result as JsonResult;
use jsonrpc_stdio_server::ServerBuilder;
use slog_scope::{info, warn};
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

fn resolve_path<I: Index>(index: &I, repository: &str) -> Result<AbsolutePathBuf> {
    index
        .get(repository)?
        .ok_or_else(|| ErrorKind::RepositoryNotInIndex.attach("Repository", repository))
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
    Err(ErrorKind::NoRepositoryForFile.attach("Path", path.display()))
}

type Tracker = HashMap<TourId, Tour>;

struct Tourist<M: TourFileManager, V: VCS, I: Index> {
    tours: Arc<RwLock<Tracker>>,
    edits: Arc<RwLock<HashSet<TourId>>>,
    manager: M,
    vcs: V,
    index: I,
}

macro_rules! tourist_ref {
    ($inst:expr, $tour_id:expr, $stop_id:expr, $tour:ident, $stop:ident) => {
        tourist_ref!($inst, $tour_id, $tour);
        let $stop = $tour
            .stops
            .iter()
            .find(|s| s.id == $stop_id)
            .ok_or_else(|| {
                ErrorKind::NoStopWithID
                    .attach("Tour ID", $tour_id.clone())
                    .attach("Stop ID", $stop_id.clone())
            })
            .as_json_result()?;
    };
    ($inst:expr, $id:expr, $tour:ident) => {
        let tours = $inst.get_tours();
        let $tour = tours
            .get(&$id)
            .ok_or_else(|| ErrorKind::NoTourWithID.attach("ID", $id.clone()))
            .as_json_result()?;
    };
}

macro_rules! tourist_ref_mut {
    ($inst:expr, $tour_id:expr, $stop_id:expr, $tour:ident, $stop:ident) => {
        tourist_ref_mut!($inst, $tour_id, $tour);
        let $stop = $tour
            .stops
            .iter_mut()
            .find(|s| s.id == $stop_id)
            .ok_or_else(|| {
                ErrorKind::NoStopWithID
                    .attach("Tour ID", $tour_id.clone())
                    .attach("Stop ID", $stop_id.clone())
            })
            .as_json_result()?;
    };
    ($inst:expr, $id:expr, $tour:ident) => {
        let mut tours = $inst.get_tours_mut();
        let $tour = tours
            .get_mut(&$id)
            .ok_or_else(|| ErrorKind::NoTourWithID.attach("ID", $id.clone()))
            .as_json_result()?;
    };
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

    /// Determines if a particular tour is editable. In the event of a `PoisonError`, we panic.
    fn is_editable(&self, tour_id: &str) -> bool {
        self.edits.read().unwrap().contains(tour_id)
    }

    /// Sets whether or not a particular tour is editable. In the event of a `PoisonError`, we
    /// panic.
    fn set_editable(&self, tour_id: TourId, edit: bool) {
        if edit {
            self.edits.write().unwrap().insert(tour_id);
        } else {
            self.edits.write().unwrap().remove(&tour_id);
        }
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
                        ErrorKind::NoStopWithID
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

    fn is_up_to_date(&self, tour_id: &str) -> Result<bool> {
        let repo_up_to_date = |(repo_name, tour_v): (&String, &String)| -> Result<bool> {
            let path = self
                .index
                .get(repo_name)?
                .ok_or_else(|| ErrorKind::RepositoryNotInIndex.attach("repo", repo_name))?;
            let curr_v = self.vcs.get_current_version(path.as_absolute_path())?;
            Ok(tour_v == &curr_v && !self.vcs.is_workspace_dirty(path.as_absolute_path())?)
        };

        let tours = self.get_tours();
        let tour = tours
            .get(tour_id)
            .ok_or_else(|| ErrorKind::NoTourWithID.attach("ID", tour_id))?;
        let eqs = tour
            .repositories
            .iter()
            .map(repo_up_to_date)
            .collect::<Result<Vec<_>>>()?;
        Ok(eqs.into_iter().all(|x| x))
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

    fn open_tour(&self, path: PathBuf, edit: bool) -> JsonResult<TourId> {
        let tour = self.manager.load_tour(path).as_json_result()?;
        let mut tours = self.get_tours_mut();
        let id = tour.id.clone();
        tours.insert(tour.id.clone(), tour);
        if edit {
            self.set_editable(id.clone(), true);
            self.refresh_tour(id.clone(), None)?;
        }
        Ok(id)
    }

    fn freeze_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.set_editable(tour_id.clone(), false);
        self.reload_tour(tour_id)?;
        Ok(())
    }

    fn unfreeze_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.set_editable(tour_id.clone(), true);
        self.refresh_tour(tour_id, None)?;
        Ok(())
    }

    fn view_tour(&self, tour_id: TourId) -> JsonResult<TourView> {
        tourist_ref!(self, tour_id, tour);
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
            edit: self.is_editable(&tour_id),
        })
        .as_json_result()
    }

    fn edit_tour_metadata(&self, tour_id: TourId, delta: TourMetadata) -> JsonResult<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        tourist_ref_mut!(self, tour_id, tour);
        delta.apply_to(tour);
        Ok(())
    }

    fn refresh_tour(&self, tour_id: TourId, commit: Option<String>) -> JsonResult<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        tourist_ref_mut!(self, tour_id, tour);
        let mut new_versions = HashMap::new();
        for mut stop in tour.stops.iter_mut() {
            let repo_path = resolve_path(&self.index, &stop.repository).as_json_result()?;
            let tour_version = tour
                .repositories
                .get(&stop.repository)
                .ok_or_else(|| {
                    ErrorKind::NoVersionForRepository.attach("Repository", stop.repository.clone())
                })
                .as_json_result()?;
            let target_version = if let Some(commit) = commit.clone() {
                commit
            } else {
                self.vcs
                    .get_current_version(repo_path.as_absolute_path())
                    .as_json_result()?
            };
            let changes = self
                .vcs
                .diff_with_version(repo_path.as_absolute_path(), &tour_version, &target_version)
                .as_json_result()?;
            if let Some(file_changes) = changes.for_file(&stop.path) {
                if let Some(line) = file_changes.adjust_line(stop.line) {
                    stop.line = line;
                } else {
                    warn!("stop broke\nchanges:\n{:?}\n", file_changes);
                    stop.broken = Some("line was deleted".to_owned());
                }
            }
            new_versions.insert(stop.repository.clone(), target_version.clone());
        }
        tour.repositories.extend(new_versions);
        Ok(())
    }

    fn forget_tour(&self, tour_id: TourId) -> JsonResult<()> {
        let mut tours = self.get_tours_mut();
        if !tours.contains_key(&tour_id) {
            return Err(ErrorKind::NoTourWithID.attach("ID", tour_id)).as_json_result();
        }
        tours.remove(&tour_id);
        Ok(())
    }

    fn reload_tour(&self, tour_id: TourId) -> JsonResult<()> {
        let tour = self.manager.reload_tour(tour_id.clone()).as_json_result()?;
        self.tours.write().unwrap().insert(tour_id, tour);
        Ok(())
    }

    fn create_stop(
        &self,
        tour_id: TourId,
        title: String,
        path: PathBuf,
        line: usize,
    ) -> JsonResult<StopId> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        if !self.is_up_to_date(&tour_id).as_json_result()? {
            return Err(ErrorKind::TourNotUpToDate.into()).as_json_result();
        }
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
            .ok_or_else(|| ErrorKind::NoTourWithID.attach("ID", tour_id))
            .as_json_result()?;
        tour.stops.push(stop);
        tour.repositories.insert(
            repo,
            self.vcs
                .get_current_version(repo_path.as_absolute_path())
                .as_json_result()?,
        );
        Ok(id)
    }

    fn view_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<StopView> {
        tourist_ref!(self, tour_id, stop_id, tour, stop);
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
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        tourist_ref_mut!(self, tour_id, stop_id, tour, stop);
        delta.apply_to(stop);
        Ok(())
    }

    fn move_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        path: PathBuf,
        line: usize,
    ) -> JsonResult<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        if !self.is_up_to_date(&tour_id).as_json_result()? {
            return Err(ErrorKind::TourNotUpToDate.into()).as_json_result();
        }
        let (rel_path, repo, repo_path) =
            find_path_in_context(&self.index, path).as_json_result()?;
        // Two things need to happen here:
        // 1. The stop needs to be moved to the approapriate relative stop/line.
        // 2. If this change happens to modify `tour.repositories`, that needs to be handled.
        // Unfortunately, both of these operations could fail -- the stop might not exist, and the
        // new file might not be in a git repository. We wouldn't want to make one mutation, then
        // crash, and not make the other. The solution is to:
        {
            tourist_ref_mut!(self, tour_id, tour);
            // First, make sure the stop actually exists in the tour
            tour.stops
                .iter()
                .find(|s| s.id == stop_id)
                .ok_or_else(|| {
                    ErrorKind::NoStopWithID
                        .attach("Tour ID", &tour_id)
                        .attach("Stop ID", &stop_id)
                })
                .as_json_result()?;
            // Then, make the change to tour.repositories
            tour.repositories.insert(
                repo,
                self.vcs
                    .get_current_version(repo_path.as_absolute_path())
                    .as_json_result()?,
            );
        }
        // Finally, once we're sure that no more failure can occur, make the change to the stop
        tourist_ref_mut!(self, tour_id, stop_id, tour, stop);
        stop.path = rel_path;
        stop.line = line;
        stop.broken = None;
        Ok(())
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

        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }

        tourist_ref_mut!(self, tour_id, tour);
        let idx = tour
            .stops
            .iter()
            .position(|stop| stop.id == stop_id)
            .ok_or_else(|| {
                ErrorKind::NoStopWithID
                    .attach("Tour ID", &tour_id)
                    .attach("Stop ID", stop_id)
            })
            .as_json_result()? as isize;
        let end_of_list = (tour.stops.len() - 1) as isize;
        tour.stops.swap(
            usize::try_from(idx)
                .context(ErrorKind::PositionDeltaOutOfRange)
                .map_err(Error::from)
                .as_json_result()?,
            usize::try_from(clamp(idx + position_delta, 0, end_of_list))
                .context(ErrorKind::PositionDeltaOutOfRange)
                .map_err(Error::from)
                .as_json_result()?,
        );
        Ok(())
    }

    fn link_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> JsonResult<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        tourist_ref_mut!(self, tour_id, stop_id, tour, stop);
        stop.children.push(StopReference {
            tour_id: other_tour_id,
            stop_id: other_stop_id,
        });
        Ok(())
    }

    fn unlink_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> JsonResult<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        tourist_ref_mut!(self, tour_id, stop_id, tour, stop);
        stop.children
            .retain(|r| !(r.tour_id == other_tour_id && r.stop_id == other_stop_id));
        Ok(())
    }

    fn locate_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        naive: bool,
    ) -> JsonResult<Option<(PathBuf, usize)>> {
        tourist_ref!(self, tour_id, stop_id, tour, stop);
        let path = resolve_path(&self.index, &stop.repository).as_json_result()?;
        let line = if naive {
            Some(stop.line)
        } else {
            if stop.broken.is_some() {
                // broken stop, can't locate
                return Ok(None);
            }
            let version = tour
                .repositories
                .get(&stop.repository)
                .ok_or_else(|| {
                    ErrorKind::NoVersionForRepository.attach("Repository", stop.repository.clone())
                })
                .as_json_result()?;
            let changes = self
                .vcs
                .diff_with_worktree(path.as_absolute_path(), version)
                .as_json_result()?;
            if let Some(changes) = changes.for_file(&stop.path) {
                let adj = changes.adjust_line(stop.line);
                if adj.is_none() {
                    warn!("locate determined stop is broken. changes:\n{:?}", changes);
                }
                adj
            } else {
                Some(stop.line)
            }
        };
        Ok(line.map(|l| (path.join_rel(&stop.path).as_path_buf().clone(), l)))
    }

    fn remove_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        tourist_ref_mut!(self, tour_id, tour);
        let n = tour.stops.len();
        tour.stops.retain(|stop| stop.id != stop_id);
        if n == tour.stops.len() {
            // No change in length means that the stop was not deleted successfully
            return Err(ErrorKind::NoStopWithID
                .attach("Tour ID", tour_id.clone())
                .attach("Stop ID", stop_id.clone()))
            .as_json_result();
        }
        // Remove any unncessary repos
        let used_repos = tour
            .stops
            .iter()
            .map(|s| s.repository.clone())
            .collect::<HashSet<_>>();
        tour.repositories
            .retain(|repo, _| used_repos.contains(repo));
        Ok(())
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
            if self.is_editable(&tour.id) {
                self.manager.save_tour(tour.id.clone()).as_json_result()?;
            }
        }
        Ok(())
    }

    fn save_tour(&self, tour_id: TourId, path: Option<PathBuf>) -> JsonResult<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
        if let Some(path) = path {
            self.manager.set_tour_path(tour_id.clone(), path);
        }
        self.manager.save_tour(tour_id).as_json_result()?;
        Ok(())
    }

    fn delete_tour(&self, tour_id: TourId) -> JsonResult<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into()).as_json_result();
        }
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

    pub fn process(&self, init_tours: Vec<(Tour, PathBuf)>) {
        info!("running server with initial tours {:?}", init_tours);
        let mut io = jsonrpc_core::IoHandler::new();
        let path_map = init_tours
            .iter()
            .map(|(tour, path)| (tour.id.clone(), path.clone()))
            .collect::<HashMap<_, _>>();
        let tours = Arc::new(RwLock::new(
            init_tours
                .into_iter()
                .map(|(tour, _)| (tour.id.clone(), tour))
                .collect(),
        ));
        let manager =
            AsyncTourFileManager::new(Arc::clone(&tours), Arc::new(RwLock::new(path_map)));
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
