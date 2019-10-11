use crate::error::{Error, ErrorKind, Result};
use crate::index::Index;
use crate::types::path::{AbsolutePathBuf, RelativePathBuf};
use crate::types::{Stop, StopReference, Tour};
use crate::vcs::VCS;
use failure::ResultExt;
use slog_scope::{debug, info, warn};
use std::cmp;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::path::PathBuf;
use uuid::Uuid;

#[cfg(test)]
mod tests;

pub mod io;
use io::TourFileManager;

pub type TourId = String;
pub type StopId = String;

#[derive(Debug, PartialEq, Eq)]
pub struct StopMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StopReferenceView {
    /// The linked tour is available in the tracker, so tour and stop titles can be provided.
    Tracked {
        tour_id: TourId,
        tour_title: String,
        /// Null if the reference links to the root of the tour.
        stop_id: Option<StopId>,
        /// Null if the reference links to the root of the tour.
        stop_title: Option<String>,
    },
    /// The linked tour is unavailable.
    Untracked {
        tour_id: TourId,
        /// Null if the reference links to the root of the tour.
        stop_id: Option<StopId>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct StopView {
    pub title: String,
    pub description: String,
    pub repository: String,
    pub children: Vec<StopReferenceView>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TourMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TourView {
    pub title: String,
    pub description: String,
    /// A list of pairs containing `(stop_id, stop_title)`.
    pub stops: Vec<(StopId, String)>,
    /// A list of pairs containing `(repository_name, commit)`.
    pub repositories: Vec<(String, String)>,
    /// True if tour is currently in edit mode.
    pub edit: bool,
}

pub struct Engine<M: TourFileManager, V: VCS, I: Index> {
    pub tours: HashMap<TourId, Tour>,
    pub edits: HashSet<TourId>,
    pub manager: M,
    pub vcs: V,
    pub index: I,
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
            })?;
    };
    ($inst:expr, $id:expr, $tour:ident) => {
        let $tour = $inst
            .tours
            .get(&$id)
            .ok_or_else(|| ErrorKind::NoTourWithID.attach("ID", $id.clone()))?;
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
            })?;
    };
    ($inst:expr, $id:expr, $tour:ident) => {
        let $tour = $inst
            .tours
            .get_mut(&$id)
            .ok_or_else(|| ErrorKind::NoTourWithID.attach("ID", $id.clone()))?;
    };
}

impl<M: TourFileManager, V: VCS, I: Index> Engine<M, V, I> {
    fn is_editable(&self, tour_id: &str) -> bool {
        self.edits.contains(tour_id)
    }

    fn set_editable(&mut self, tour_id: TourId, edit: bool) {
        if edit {
            self.edits.insert(tour_id);
        } else {
            self.edits.remove(&tour_id);
        }
    }

    /// Determines if a tour's repositories are up to date, with clean workspaces.
    fn is_up_to_date(&self, tour_id: &str) -> Result<bool> {
        let repo_up_to_date = |(repo_name, tour_v): (&String, &String)| -> Result<bool> {
            let path = self
                .index
                .get(repo_name)?
                .ok_or_else(|| ErrorKind::RepositoryNotInIndex.attach("repo", repo_name))?;
            let curr_v = self.vcs.get_current_version(path.as_absolute_path())?;
            Ok(tour_v == &curr_v && !self.vcs.is_workspace_dirty(path.as_absolute_path())?)
        };

        tourist_ref!(self, tour_id.to_owned(), tour);
        let eqs = tour
            .repositories
            .iter()
            .map(repo_up_to_date)
            .collect::<Result<Vec<_>>>()?;
        Ok(eqs.into_iter().all(|x| x))
    }

    fn find_path_in_context(
        &self,
        path: PathBuf,
    ) -> Result<(RelativePathBuf, String, AbsolutePathBuf)> {
        let deep = AbsolutePathBuf::new(path.clone())
            .ok_or_else(|| ErrorKind::ExpectedAbsolutePath.attach("Path", path.display()))?;
        for (repo_name, repo_path) in self.index.all()? {
            if let Some(rel) = deep.try_relative(repo_path.as_absolute_path()) {
                return Ok((rel, repo_name.to_owned(), repo_path.clone()));
            }
        }
        Err(ErrorKind::NoRepositoryForFile.attach("Path", path.display()))
    }

    pub fn list_tours(&self) -> Result<Vec<(TourId, String)>> {
        info!("called Engine::list_tours");
        Ok(self
            .tours
            .values()
            .map(|tour| (tour.id.clone(), tour.title.clone()))
            .collect())
    }

    pub fn create_tour(&mut self, title: String) -> Result<TourId> {
        info!(
            "called Engine::create_tour with args: {{ title: {} }}",
            &title,
        );
        let id = format!("{}", Uuid::new_v4().to_simple());
        let new_tour = Tour {
            protocol_version: "1.0".to_owned(),
            id: id.clone(),
            title,
            description: "".to_owned(),
            stops: Vec::new(),
            repositories: HashMap::new(),
        };
        debug!("new tour with id: {}", &id);
        self.tours.insert(id.clone(), new_tour);
        Ok(id)
    }

    pub fn open_tour(&mut self, path: PathBuf, edit: bool) -> Result<TourId> {
        info!(
            "called Engine::open_tour with args: {{ path: {}, edit: {} }}",
            path.display(),
            edit,
        );
        let tour = self.manager.load_tour(path)?;
        let id = tour.id.clone();
        self.tours.insert(tour.id.clone(), tour);
        if edit {
            self.set_editable(id.clone(), true);
        }
        Ok(id)
    }

    pub fn freeze_tour(&mut self, tour_id: TourId) -> Result<()> {
        self.set_editable(tour_id.clone(), false);
        self.reload_tour(tour_id)?;
        Ok(())
    }

    pub fn unfreeze_tour(&mut self, tour_id: TourId) -> Result<()> {
        self.set_editable(tour_id.clone(), true);
        Ok(())
    }

    pub fn view_tour(&self, tour_id: TourId) -> Result<TourView> {
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
    }

    pub fn edit_tour_metadata(&mut self, tour_id: TourId, mut delta: TourMetadata) -> Result<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        tourist_ref_mut!(self, tour_id, tour);
        if let Some(title) = delta.title.take() {
            tour.title = title;
        }
        if let Some(description) = delta.description.take() {
            tour.description = description;
        }
        Ok(())
    }

    pub fn refresh_tour(&mut self, tour_id: TourId) -> Result<()> {
        info!(
            "called Engine::refresh_tour with args: {{ tour_id: {} }}",
            &tour_id,
        );
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        tourist_ref_mut!(self, tour_id, tour);
        let mut new_versions = HashMap::new();
        for (repo_name, tour_version) in &tour.repositories {
            debug!("refreshing {} in tour {}", repo_name, &tour_id);
            let repo_path = self
                .index
                .get(repo_name)?
                .ok_or_else(|| ErrorKind::RepositoryNotInIndex.attach("Repository", repo_name))?;
            let target_version = self.vcs.get_current_version(repo_path.as_absolute_path())?;
            let changes = self.vcs.diff_with_version(
                repo_path.as_absolute_path(),
                tour_version,
                &target_version,
            )?;
            for stop in tour.stops.iter_mut().filter(|s| s.repository == *repo_name) {
                if let Some(file_changes) = changes.for_file(&stop.path) {
                    if let Some(line) = file_changes.adjust_line(stop.line) {
                        stop.line = line;
                    } else {
                        warn!("stop {} broke. changes:\n{:?}\n", &stop.id, file_changes);
                        stop.broken = Some("line was deleted".to_owned());
                    }
                }
            }
            new_versions.insert(repo_name.clone(), target_version);
        }
        tour.repositories.extend(new_versions);
        Ok(())
    }

    pub fn forget_tour(&mut self, tour_id: TourId) -> Result<()> {
        if !self.tours.contains_key(&tour_id) {
            return Err(ErrorKind::NoTourWithID.attach("ID", tour_id));
        }
        self.tours.remove(&tour_id);
        Ok(())
    }

    pub fn reload_tour(&mut self, tour_id: TourId) -> Result<()> {
        let tour = self.manager.reload_tour(tour_id.clone())?;
        self.tours.insert(tour_id, tour);
        Ok(())
    }

    pub fn create_stop(
        &mut self,
        tour_id: TourId,
        title: String,
        path: PathBuf,
        line: usize,
    ) -> Result<StopId> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        if !self.is_up_to_date(&tour_id)? {
            return Err(ErrorKind::TourNotUpToDate.into());
        }
        let id = format!("{}", Uuid::new_v4().to_simple());
        let (rel_path, repo, repo_path) = self.find_path_in_context(path)?;
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
        tourist_ref_mut!(self, tour_id, tour);
        tour.stops.push(stop);
        tour.repositories.insert(
            repo,
            self.vcs.get_current_version(repo_path.as_absolute_path())?,
        );
        Ok(id)
    }

    pub fn view_stop(&self, tour_id: TourId, stop_id: StopId) -> Result<StopView> {
        let view_stop_reference = |sr: &StopReference| -> Result<StopReferenceView> {
            let other_tour = self.tours.get(&sr.tour_id);
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
        };

        tourist_ref!(self, tour_id, stop_id, tour, stop);
        Ok(StopView {
            title: stop.title.clone(),
            description: stop.description.clone(),
            repository: stop.repository.clone(),
            children: stop
                .children
                .iter()
                .map(view_stop_reference)
                .collect::<Result<Vec<_>>>()?,
        })
    }

    pub fn edit_stop_metadata(
        &mut self,
        tour_id: TourId,
        stop_id: StopId,
        mut delta: StopMetadata,
    ) -> Result<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        tourist_ref_mut!(self, tour_id, stop_id, tour, stop);
        if let Some(title) = delta.title.take() {
            stop.title = title;
        }
        if let Some(description) = delta.description.take() {
            stop.description = description;
        }
        Ok(())
    }

    pub fn move_stop(
        &mut self,
        tour_id: TourId,
        stop_id: StopId,
        path: PathBuf,
        line: usize,
    ) -> Result<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        if !self.is_up_to_date(&tour_id)? {
            return Err(ErrorKind::TourNotUpToDate.into());
        }
        let (rel_path, repo, repo_path) = self.find_path_in_context(path)?;
        // Two things need to happen here:
        // 1. The stop needs to be moved to the approapriate relative stop/line.
        // 2. If this change happens to modify `tour.repositories`, that needs to be handled.
        // Unfortunately, both of these operations could fail -- the stop might not exist, and the
        // new file might not be in a git repository. We wouldn't want to make one mutation, then
        // crash, and not make the other. The solution is to:
        {
            tourist_ref_mut!(self, tour_id, tour);
            // First, make sure the stop actually exists in the tour
            tour.stops.iter().find(|s| s.id == stop_id).ok_or_else(|| {
                ErrorKind::NoStopWithID
                    .attach("Tour ID", &tour_id)
                    .attach("Stop ID", &stop_id)
            })?;
            // Then, make the change to tour.repositories
            tour.repositories.insert(
                repo,
                self.vcs.get_current_version(repo_path.as_absolute_path())?,
            );
        }
        // Finally, once we're sure that no more failure can occur, make the change to the stop
        tourist_ref_mut!(self, tour_id, stop_id, tour, stop);
        stop.path = rel_path;
        stop.line = line;
        stop.broken = None;
        Ok(())
    }

    pub fn reorder_stop(
        &mut self,
        tour_id: TourId,
        stop_id: StopId,
        position_delta: isize,
    ) -> Result<()> {
        // Clamp `val` to be within `min` and `max` inclusive. Assumes `min <= max`
        pub fn clamp(val: isize, min: isize, max: isize) -> isize {
            cmp::min(cmp::max(val, min), max)
        }

        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
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
            })? as isize;
        let end_of_list = (tour.stops.len() - 1) as isize;
        tour.stops.swap(
            usize::try_from(idx)
                .context(ErrorKind::PositionDeltaOutOfRange)
                .map_err(Error::from)?,
            usize::try_from(clamp(idx + position_delta, 0, end_of_list))
                .context(ErrorKind::PositionDeltaOutOfRange)
                .map_err(Error::from)?,
        );
        Ok(())
    }

    pub fn link_stop(
        &mut self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> Result<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        tourist_ref_mut!(self, tour_id, stop_id, tour, stop);
        stop.children.push(StopReference {
            tour_id: other_tour_id,
            stop_id: other_stop_id,
        });
        Ok(())
    }

    pub fn unlink_stop(
        &mut self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> Result<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        tourist_ref_mut!(self, tour_id, stop_id, tour, stop);
        stop.children
            .retain(|r| !(r.tour_id == other_tour_id && r.stop_id == other_stop_id));
        Ok(())
    }

    pub fn locate_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        naive: bool,
    ) -> Result<Option<(PathBuf, usize)>> {
        tourist_ref!(self, tour_id, stop_id, tour, stop);
        let path = self.index.get(&stop.repository)?.ok_or_else(|| {
            ErrorKind::RepositoryNotInIndex.attach("Repository", &stop.repository)
        })?;
        let line = if naive {
            Some(stop.line)
        } else {
            if stop.broken.is_some() {
                // broken stop, can't locate
                return Ok(None);
            }
            let version = tour.repositories.get(&stop.repository).ok_or_else(|| {
                ErrorKind::NoVersionForRepository.attach("Repository", stop.repository.clone())
            })?;
            let changes = self
                .vcs
                .diff_with_worktree(path.as_absolute_path(), version)?;
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

    pub fn remove_stop(&mut self, tour_id: TourId, stop_id: StopId) -> Result<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        tourist_ref_mut!(self, tour_id, tour);
        let n = tour.stops.len();
        tour.stops.retain(|stop| stop.id != stop_id);
        if n == tour.stops.len() {
            // No change in length means that the stop was not deleted successfully
            return Err(ErrorKind::NoStopWithID
                .attach("Tour ID", tour_id.clone())
                .attach("Stop ID", stop_id.clone()));
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

    pub fn index_repository(&mut self, repo_name: String, path: Option<PathBuf>) -> Result<()> {
        if let Some(path) = path {
            let abs_path = AbsolutePathBuf::new(path.clone())
                .ok_or_else(|| ErrorKind::ExpectedAbsolutePath.attach("Path", path.display()))?;
            self.index.set(&repo_name, &abs_path)
        } else {
            self.index.unset(&repo_name)
        }
    }

    pub fn save_tour(&mut self, tour_id: TourId, path: Option<PathBuf>) -> Result<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        if let Some(path) = path {
            self.manager.set_tour_path(tour_id.clone(), path);
        }
        tourist_ref!(self, tour_id, tour);
        self.manager.save_tour(&tour)?;
        Ok(())
    }

    pub fn delete_tour(&mut self, tour_id: TourId) -> Result<()> {
        if !self.is_editable(&tour_id) {
            return Err(ErrorKind::TourNotEditable.into());
        }
        self.forget_tour(tour_id.clone())?;
        self.manager.delete_tour(tour_id)?;
        Ok(())
    }

    pub fn checkout_for_tour(&self, tour_id: TourId) -> Result<()> {
        tourist_ref!(self, tour_id, tour);
        for (repo_name, version) in tour.repositories.iter() {
            let path = self
                .index
                .get(&repo_name)?
                .ok_or_else(|| ErrorKind::RepositoryNotInIndex.attach("repo", repo_name.clone()))?;
            self.vcs
                .checkout_version(path.as_absolute_path(), &version)?;
        }
        Ok(())
    }
}
