use crate::error::{Error, Result};
use crate::resolve;
use std::collections::HashMap;
use tourist_types::{Index, Stop, Tour};

fn stop_to_markdown(
    stop: &Stop,
    ctx_args: &Option<(&Index, usize, usize)>,
    commit_map: &HashMap<String, String>,
) -> Result<String> {
    let position = format!(
        "{}:{} - line {}",
        stop.repository,
        stop.path.as_path_buf().display(),
        stop.line
    );
    let mut lines = "".to_owned();
    if let Some((index, above, below)) = ctx_args {
        let repo_path = index
            .get(&stop.repository)
            .ok_or_else(|| Error::NotInIndex(stop.repository.clone()))?;

        let low = stop.line - above;
        let hi = stop.line + below;
        let content = resolve::lookup_file_contents(
            repo_path.as_absolute_path(),
            commit_map
                .get(&stop.repository)
                .ok_or_else(|| Error::NoCommitForRepository(stop.repository.to_owned()))?,
            &stop.path,
        )?
        .lines()
        .enumerate()
        .filter_map(|(i, e)| {
            if i + 1 == stop.line {
                Some(format!(" -> {}", e))
            } else if low <= i + 1 && i < hi {
                Some(format!("    {}", e))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
        lines = format!("\n\n```\n{}\n```", content);
    }
    Ok(format!(
        "## {}\n*{}*\n\n{}{}",
        stop.title, position, stop.body, lines
    ))
}

fn tour_to_markdown(tour: &Tour, ctx_args: &Option<(&Index, usize, usize)>) -> Result<String> {
    let repos = tour
        .repositories
        .iter()
        .map(|(r, c)| format!("{} is on commit {}", r, c))
        .collect::<Vec<_>>()
        .join("\n");
    let stops = tour
        .stops
        .iter()
        .map(|stop| stop_to_markdown(&stop, ctx_args, &tour.repositories))
        .collect::<Result<Vec<_>>>()?
        .join("\n\n-----\n\n");
    Ok(format!(
        "# {}\n\n{}\n\n# Stops\n\n{}\n\n# Repositories\n\n{}",
        tour.title, tour.description, stops, repos,
    ))
}

pub fn dump_tour(ctx_args: &Option<(&Index, usize, usize)>, tour: &Tour) -> Result<()> {
    let md = tour_to_markdown(&tour, ctx_args)?;
    println!("{}", md);
    Ok(())
}
