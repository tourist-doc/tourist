use jsonrpc_core::Result as JsonResult;
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type TourId = String;
pub type StopId = String;

/// Metadata for a tour stop.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StopMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// A view of a tour stop reference.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
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

/// A view of a tour stop.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StopView {
    pub title: String,
    pub description: String,
    pub repository: String,
    pub children: Vec<StopReferenceView>,
}

/// Metadata for a tour.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TourMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// A view of a tour.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
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

/// The main RPC interface provided by `tourist serve`.
///
/// Running `tourist serve` will provide a JSONRPC 2.0 interface via stdio. Interacting with the
/// API is much the same as interacting with a Language Server operating on the
/// [Language Server Protocol](https://microsoft.github.io/language-server-protocol/).
///
/// The provided endpoints should be all you need to create a rich extension to any modern editor.
/// The server handles file IO, complex state, and other potential sources of complexity, allowing
/// your editor plugin to be simple and straightforward.
///
/// # API Usage
/// A JSONRPC 2.0 call looks like:
/// ```json
/// {
///     "jsonrpc": "2.0",
///     "method": "<method_name>",
///     "params": [<param_1>, <param_2>, ...],
///     "id": <id_number>
/// }
/// ```
/// The ID number simply identifies a call so the response can be matched accordingly. You can read
/// more about JSONRPC 2.0 [here](https://www.jsonrpc.org/specification).
#[rpc]
pub trait TouristRpc {
    /// List all tours that are currently open, along with their titles.
    #[rpc(name = "list_tours")]
    fn rpc_list_tours(&self) -> JsonResult<Vec<(TourId, String)>>;

    /// Create a new tour and open it in edit mode. Returns the new tour's ID.
    #[rpc(name = "create_tour")]
    fn rpc_create_tour(&self, title: String) -> JsonResult<TourId>;

    /// Open an existing tour from disk. If `edit` is true, the tour will be available for editing.
    /// Returns the opened tour's ID.
    #[rpc(name = "open_tour")]
    fn rpc_open_tour(&self, path: PathBuf, edit: bool) -> JsonResult<TourId>;

    /// Sets a tour as uneditable.
    #[rpc(name = "freeze_tour")]
    fn rpc_freeze_tour(&self, tour_id: TourId) -> JsonResult<()>;

    /// Sets a tour as editable.
    #[rpc(name = "unfreeze_tour")]
    fn rpc_unfreeze_tour(&self, tour_id: TourId) -> JsonResult<()>;

    /// View all of the top-level data for a tour.
    #[rpc(name = "view_tour")]
    fn rpc_view_tour(&self, tour_id: TourId) -> JsonResult<TourView>;

    /// Edit tour metadata, e.g. title and description. The delta object has a number of optional
    /// fields; those that are set will be applied.
    #[rpc(name = "edit_tour_metadata")]
    fn rpc_edit_tour_metadata(&self, tour_id: TourId, delta: TourMetadata) -> JsonResult<()>;

    /// Remove a tour from the list of tracked tours. If you would like to delete the tour from disk
    /// as well, use `delete_tour`.
    #[rpc(name = "forget_tour")]
    fn rpc_forget_tour(&self, tour_id: TourId) -> JsonResult<()>;

    /// Reset any edits made to a tour from its backing file.
    #[rpc(name = "reset_tour")]
    fn rpc_reload_tour(&self, tour_id: TourId) -> JsonResult<()>;

    /// Create a new stop in the given tour. Returns the ID of the new stop.
    #[rpc(name = "create_stop")]
    fn rpc_create_stop(
        &self,
        tour_id: TourId,
        title: String,
        path: PathBuf,
        line: usize,
    ) -> JsonResult<StopId>;

    /// View all of the top-level data for a stop.
    #[rpc(name = "view_stop")]
    fn rpc_view_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<StopView>;

    /// Edit stop metadata, e.g. title and description. The delta object has a number of optional
    /// fields; those that are set will be applied.
    #[rpc(name = "edit_stop_metadata")]
    fn rpc_edit_stop_metadata(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        delta: StopMetadata,
    ) -> JsonResult<()>;

    /// Move a stop to a different place in the codebase.
    #[rpc(name = "move_stop")]
    fn rpc_move_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        path: PathBuf,
        line: usize,
    ) -> JsonResult<()>;

    /// Change the order of a tour's stops. Position delta is applied to the position of the stop in
    /// the list, bounded by the length of the list.
    #[rpc(name = "reorder_stop")]
    fn rpc_reorder_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        position_delta: isize,
    ) -> JsonResult<()>;

    /// Link a tour stop to another tour or tour stop. If `other_stop_id` is `None`, the link will
    /// go to the tour's landing page. Otherwise the link will go to the stop itself.
    #[rpc(name = "link_stop")]
    fn rpc_link_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> JsonResult<()>;

    /// Unlink a tour stop from another tour or tour stop.
    #[rpc(name = "unlink_stop")]
    fn rpc_unlink_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        other_tour_id: TourId,
        other_stop_id: Option<StopId>,
    ) -> JsonResult<()>;

    /// Find the file location for a given stop. If `naive` is set, the location will be provided
    /// directly from the tour file, with no adjustment; otherwise the location will be adjusted
    /// based on a git diff.
    #[rpc(name = "locate_stop")]
    fn rpc_locate_stop(
        &self,
        tour_id: TourId,
        stop_id: StopId,
        naive: bool,
    ) -> JsonResult<Option<(PathBuf, usize)>>;

    /// Remove a stop from an open tour.
    #[rpc(name = "remove_stop")]
    fn rpc_remove_stop(&self, tour_id: TourId, stop_id: StopId) -> JsonResult<()>;

    /// Refresh a tour's stops to the provided commit. If no commit is provided, HEAD is used.
    #[rpc(name = "refresh_tour")]
    fn rpc_refresh_tour(&self, tour_id: TourId) -> JsonResult<()>;

    /// Save a tour to disk. If the tour is new, a path must be provided; otherwise the path can be
    /// left empty.
    #[rpc(name = "save_tour")]
    fn rpc_save_tour(&self, tour_id: TourId, path: Option<PathBuf>) -> JsonResult<()>;

    /// Remove a tour from the tracker and delete it from disk.
    #[rpc(name = "delete_tour")]
    fn rpc_delete_tour(&self, tour_id: TourId) -> JsonResult<()>;

    /// Update the repository index, mapping a name to a path. If a null value is passed instead of
    /// a path, the name is removed from the index instead.
    #[rpc(name = "index_repository")]
    fn rpc_index_repository(&self, repo_name: String, path: Option<PathBuf>) -> JsonResult<()>;

    /// Check out the appropriate version of each of the tour's repositories.
    #[rpc(name = "checkout_for_tour")]
    fn rpc_checkout_for_tour(&self, tour_id: TourId) -> JsonResult<()>;
}
