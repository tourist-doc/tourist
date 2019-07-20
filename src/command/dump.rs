use crate::error::{Error, Result};
use crate::types::{Index, Stop, Tour};
use crate::vcs::VCS;

pub enum Dump<V: VCS> {
    Context {
        vcs: V,
        index: Index,
        above: usize,
        below: usize,
    },
    NoContext,
}

fn code_range(code: String, target: usize, above: usize, below: usize) -> String {
    let low = if above <= target { target - above } else { 0 };
    let hi = target + below;
    code.lines()
        .enumerate()
        .filter_map(|(i, e)| {
            if i + 1 == target {
                Some(format!(" -> {}", e))
            } else if low <= i + 1 && i < hi {
                Some(format!("    {}", e))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl<V: VCS> Dump<V> {
    pub fn new() -> Self {
        Dump::NoContext
    }

    pub fn with_context(vcs: V, index: Index, above: usize, below: usize) -> Self {
        Dump::Context {
            vcs,
            index,
            above,
            below,
        }
    }

    fn extract_context(&self, stop: &Stop, commit: &str) -> Result<String> {
        match self {
            Dump::Context {
                vcs,
                index,
                above,
                below,
            } => {
                let repo_path = index
                    .get(&stop.repository)
                    .ok_or_else(|| Error::NotInIndex(stop.repository.clone()))?;

                let content = code_range(
                    vcs.lookup_file_contents(repo_path.as_absolute_path(), commit, &stop.path)?,
                    stop.line,
                    *above,
                    *below,
                );
                Ok(format!("\n\n```\n{}\n```", content))
            }
            Dump::NoContext => Ok("".to_owned()),
        }
    }

    fn process_stop(&self, stop: &Stop, commit: &str) -> Result<String> {
        let position = format!(
            "{}:{} - line {}",
            stop.repository,
            stop.path.as_path_buf().display(),
            stop.line
        );
        Ok(format!(
            "## {}\n*{}*\n\n{}{}",
            stop.title,
            position,
            stop.description,
            self.extract_context(stop, commit)?
        ))
    }

    pub fn process(&self, tour: &Tour) -> Result<()> {
        let repos = tour
            .repositories
            .iter()
            .map(|(r, c)| format!("{} is on commit {}", r, c))
            .collect::<Vec<_>>()
            .join("\n");
        let stops = tour
            .stops
            .iter()
            .map(|stop| {
                let commit = tour
                    .repositories
                    .get(&stop.repository)
                    .ok_or_else(|| Error::NoCommitForRepository(stop.repository.to_owned()))?;
                self.process_stop(&stop, &commit)
            })
            .collect::<Result<Vec<_>>>()?
            .join("\n\n-----\n\n");
        let md = format!(
            "# {}\n\n{}\n\n# Stops\n\n{}\n\n# Repositories\n\n{}",
            tour.title, tour.description, stops, repos
        );
        println!("{}", md);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::code_range;

    #[test]
    fn extract_context_works() {
        assert_eq!(
            "    1\n -> 2\n    3".to_owned(),
            code_range("1\n2\n3".to_owned(), 2, 1, 1)
        );

        assert_eq!(
            " -> 2".to_owned(),
            code_range("1\n2\n3".to_owned(), 2, 0, 0)
        );

        assert_eq!(
            "    1\n -> 2\n    3".to_owned(),
            code_range("1\n2\n3".to_owned(), 2, 10, 6)
        );
    }
}
