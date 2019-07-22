use crate::types;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;

pub const PROTOCOL_VERSION: &str = "1.0";

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Child {
    pub tour_id: String,
    pub stop_num: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub id: String,
    pub title: String,
    pub body: String,
    pub rel_path: String,
    pub repository: String,
    pub line: usize,
    pub child_stops: Vec<Child>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub repository: String,
    pub commit: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TourFile {
    pub protocol_version: String,
    pub id: String,
    pub title: String,
    pub description: String,
    pub stops: Vec<Stop>,
    pub repositories: Vec<Repository>,
    pub generator: Option<usize>,
}

impl TryFrom<&str> for TourFile {
    type Error = serde_json::Error;
    fn try_from(tf: &str) -> Result<TourFile, Self::Error> {
        serde_json::from_str(tf)
    }
}

impl fmt::Display for TourFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(self).or(Err(fmt::Error))?)
    }
}

impl Into<types::Tour> for TourFile {
    fn into(self) -> types::Tour {
        types::Tour {
            protocol_version: self.protocol_version,
            generator: self.generator.unwrap_or(0),
            id: self.id,
            title: self.title,
            description: self.description,
            stops: self
                .stops
                .into_iter()
                .map(|stop| types::Stop {
                    id: stop.id,
                    title: stop.title,
                    description: stop.body,
                    path: stop.rel_path.as_str().replace("\\", "/").into(),
                    repository: stop.repository,
                    line: stop.line,
                    children: stop
                        .child_stops
                        .into_iter()
                        .map(|c| types::StopReference {
                            tour_id: c.tour_id,
                            stop_id: None,
                        })
                        .collect::<Vec<_>>(),
                })
                .collect::<Vec<_>>(),
            repositories: self
                .repositories
                .iter()
                .map(|r| (r.repository.to_owned(), r.commit.to_owned()))
                .collect::<HashMap<_, _>>(),
        }
    }
}

impl From<types::Tour> for TourFile {
    fn from(tour: types::Tour) -> Self {
        TourFile {
            protocol_version: tour.protocol_version,
            generator: Some(tour.generator),
            id: tour.id,
            title: tour.title,
            description: tour.description,
            stops: tour
                .stops
                .into_iter()
                .map(|stop| Stop {
                    id: stop.id,
                    title: stop.title,
                    body: stop.description,
                    rel_path: stop.path.as_git_path(),
                    repository: stop.repository,
                    line: stop.line,
                    child_stops: stop
                        .children
                        .into_iter()
                        .map(|c| Child {
                            tour_id: c.tour_id,
                            stop_num: 0,
                        })
                        .collect::<Vec<_>>(),
                })
                .collect::<Vec<_>>(),
            repositories: tour
                .repositories
                .into_iter()
                .map(|(r, c)| Repository {
                    repository: r,
                    commit: c,
                })
                .collect::<Vec<_>>(),
        }
    }
}
