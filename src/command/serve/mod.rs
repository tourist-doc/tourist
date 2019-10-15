use crate::engine::io::{BasicTourFileManager, TourFileManager};
use crate::engine::*;
use crate::error::AsJsonResult;
use crate::index::Index;
use crate::serialize::jsonrpc;
use crate::serialize::jsonrpc::TouristRpc;
use crate::types::Tour;
use crate::vcs::VCS;
use jsonrpc_core;
use jsonrpc_core::Result as JsonResult;
use jsonrpc_stdio_server::ServerBuilder;
use slog_scope::info;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

impl<
        M: TourFileManager + Send + Sync + 'static,
        V: VCS + Send + Sync + 'static,
        I: Index + Send + Sync + 'static,
    > TouristRpc for Arc<RwLock<Engine<M, V, I>>>
{
    fn rpc_list_tours(&self) -> JsonResult<Vec<(TourId, String)>> {
        self.read().unwrap().list_tours().as_json_result()
    }

    fn rpc_create_tour(&self, title: String) -> JsonResult<TourId> {
        self.write().unwrap().create_tour(title).as_json_result()
    }

    fn rpc_open_tour(&self, path: PathBuf, edit: bool) -> JsonResult<TourId> {
        self.write().unwrap().open_tour(path, edit).as_json_result()
    }

    fn rpc_freeze_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.write().unwrap().freeze_tour(tour_id).as_json_result()
    }

    fn rpc_unfreeze_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.write()
            .unwrap()
            .unfreeze_tour(tour_id)
            .as_json_result()
    }

    fn rpc_view_tour(&self, tour_id: TourId) -> JsonResult<jsonrpc::TourView> {
        let view = self.read().unwrap().view_tour(tour_id).as_json_result()?;
        Ok(jsonrpc::TourView {
            title: view.title,
            description: view.description,
            stops: view.stops,
            repositories: view.repositories,
            edit: view.edit,
            up_to_date: view.up_to_date,
        })
    }

    fn rpc_edit_tour_metadata(
        &self,
        tour_id: TourId,
        delta: jsonrpc::TourMetadata,
    ) -> JsonResult<()> {
        let delta = TourMetadata {
            title: delta.title,
            description: delta.description,
        };
        self.write()
            .unwrap()
            .edit_tour_metadata(tour_id, delta)
            .as_json_result()
    }

    fn rpc_refresh_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.write().unwrap().refresh_tour(tour_id).as_json_result()
    }

    fn rpc_forget_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.write().unwrap().forget_tour(tour_id).as_json_result()
    }

    fn rpc_reload_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.write().unwrap().reload_tour(tour_id).as_json_result()
    }

    fn rpc_create_stop(
        &self,
        tour_id: TourId,
        title: String,
        path: PathBuf,
        line: usize,
    ) -> JsonResult<StopId> {
        self.write()
            .unwrap()
            .create_stop(tour_id, title, path, line)
            .as_json_result()
    }

    fn rpc_view_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<jsonrpc::StopView> {
        let view = self
            .read()
            .unwrap()
            .view_stop(tour_id, stop_id)
            .as_json_result()?;
        Ok(jsonrpc::StopView {
            title: view.title,
            description: view.description,
            repository: view.repository,
            children: view
                .children
                .into_iter()
                .map(|child| match child {
                    StopReferenceView::Tracked {
                        tour_id,
                        tour_title,
                        stop_id,
                        stop_title,
                    } => jsonrpc::StopReferenceView::Tracked {
                        tour_id,
                        tour_title,
                        stop_id,
                        stop_title,
                    },
                    StopReferenceView::Untracked { tour_id, stop_id } => {
                        jsonrpc::StopReferenceView::Untracked { tour_id, stop_id }
                    }
                })
                .collect::<Vec<_>>(),
        })
    }

    fn rpc_edit_stop_metadata(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        delta: jsonrpc::StopMetadata,
    ) -> JsonResult<()> {
        let delta = StopMetadata {
            title: delta.title,
            description: delta.description,
        };
        self.write()
            .unwrap()
            .edit_stop_metadata(tour_id, stop_id, delta)
            .as_json_result()
    }

    fn rpc_move_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        path: PathBuf,
        line: usize,
    ) -> JsonResult<()> {
        self.write()
            .unwrap()
            .move_stop(tour_id, stop_id, path, line)
            .as_json_result()
    }

    fn rpc_reorder_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        position_delta: isize,
    ) -> JsonResult<()> {
        self.write()
            .unwrap()
            .reorder_stop(tour_id, stop_id, position_delta)
            .as_json_result()
    }

    fn rpc_link_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> JsonResult<()> {
        self.write()
            .unwrap()
            .link_stop(tour_id, stop_id, other_tour_id, other_stop_id)
            .as_json_result()
    }

    fn rpc_unlink_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> JsonResult<()> {
        self.write()
            .unwrap()
            .unlink_stop(tour_id, stop_id, other_tour_id, other_stop_id)
            .as_json_result()
    }

    fn rpc_locate_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        naive: bool,
    ) -> JsonResult<Option<(PathBuf, usize)>> {
        self.read()
            .unwrap()
            .locate_stop(tour_id, stop_id, naive)
            .as_json_result()
    }

    fn rpc_remove_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<()> {
        self.write()
            .unwrap()
            .remove_stop(tour_id, stop_id)
            .as_json_result()
    }

    fn rpc_index_repository(&self, repo_name: String, path: Option<PathBuf>) -> JsonResult<()> {
        self.write()
            .unwrap()
            .index_repository(repo_name, path)
            .as_json_result()
    }

    fn rpc_save_tour(&self, tour_id: TourId, path: Option<PathBuf>) -> JsonResult<()> {
        self.write()
            .unwrap()
            .save_tour(tour_id, path)
            .as_json_result()
    }

    fn rpc_delete_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.write().unwrap().delete_tour(tour_id).as_json_result()
    }

    fn rpc_checkout_for_tour(&self, tour_id: TourId) -> JsonResult<()> {
        self.write()
            .unwrap()
            .checkout_for_tour(tour_id)
            .as_json_result()
    }
}

pub struct Serve<V: VCS + Send + Sync + 'static, I: Index + Send + Sync + 'static> {
    vcs: V,
    index: I,
}

impl<V: VCS + Send + Sync + 'static, I: Index + Send + Sync + 'static> Serve<V, I> {
    pub fn new(vcs: V, index: I) -> Self {
        Serve { vcs, index }
    }

    pub fn process(self, init_tours: Vec<(Tour, PathBuf)>) {
        info!("running server with initial tours {:?}", init_tours);
        let mut io = jsonrpc_core::IoHandler::new();
        let path_map = init_tours
            .iter()
            .map(|(tour, path)| (tour.id.clone(), path.clone()))
            .collect::<HashMap<_, _>>();
        let tours = init_tours
            .into_iter()
            .map(|(tour, _)| (tour.id.clone(), tour))
            .collect::<HashMap<_, _>>();
        let manager = BasicTourFileManager::new(path_map);
        io.extend_with(
            Arc::new(RwLock::new(Engine {
                tours,
                manager,
                vcs: self.vcs,
                index: self.index,
                edits: HashSet::new(),
            }))
            .to_delegate(),
        );
        info!("starting tourist server");
        ServerBuilder::new(io).build();
    }
}
