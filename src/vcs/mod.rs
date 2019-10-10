use crate::error::{Error, ErrorKind, Result};
use crate::types::path::{AbsolutePath, RelativePathBuf};
use failure::ResultExt;
use git2::{DiffOptions, Oid, Repository};

mod changes;

pub use changes::{Changes, FileChanges, LineChanges};
use changes::{DiffFileEvent, DiffLineEvent};

pub trait VCS: Send + Sync + 'static + Clone {
    fn get_current_version(&self, repo_path: AbsolutePath<'_>) -> Result<String>;

    fn diff_with_version(
        &self,
        repo_path: AbsolutePath<'_>,
        from: &str,
        to: &str,
    ) -> Result<Changes>;

    fn diff_with_worktree(&self, repo_path: AbsolutePath<'_>, from: &str) -> Result<Changes>;

    fn is_workspace_dirty(&self, repo_path: AbsolutePath<'_>) -> Result<bool>;

    fn lookup_file_bytes(
        &self,
        repo_path: AbsolutePath<'_>,
        commit: &str,
        file_path: &RelativePathBuf,
    ) -> Result<Vec<u8>>;

    fn lookup_file_contents(
        &self,
        repo_path: AbsolutePath<'_>,
        commit: &str,
        file_path: &RelativePathBuf,
    ) -> Result<String> {
        let content = self.lookup_file_bytes(repo_path, commit, file_path)?;
        Ok(std::str::from_utf8(&content)
            .context(ErrorKind::EncodingFailure)?
            .to_owned())
    }
}

#[derive(Clone)]
pub struct Git;

impl Git {
    fn diff(&self, repo_path: AbsolutePath<'_>, from: &str, to: Option<&str>) -> Result<Changes> {
        let repo = Repository::open(repo_path.as_path())
            .context(ErrorKind::InvalidRepositoryPath)
            .or_else(|e| {
                Err(Error::from(e)
                    .attach("repo_path", format!("{}", repo_path.as_path().display())))
            })?;

        let from_tree = Oid::from_str(from)
            .and_then(|oid| repo.find_commit(oid)?.tree())
            .context(ErrorKind::InvalidCommitHash)?;
        let mut opts = DiffOptions::new();
        opts.minimal(true);
        opts.ignore_whitespace_eol(true);

        let diff = if let Some(to) = to {
            let to_tree = Oid::from_str(to)
                .and_then(|oid| repo.find_commit(oid)?.tree())
                .context(ErrorKind::InvalidCommitHash)?;
            repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut opts))
                .context(ErrorKind::DiffFailed)?
        } else {
            repo.diff_tree_to_workdir(Some(&from_tree), Some(&mut opts))
                .context(ErrorKind::DiffFailed)?
        };

        let mut file_events = vec![];
        let mut line_events = vec![];
        diff.foreach(
            &mut |delta, _| {
                if let Some(r) = delta.old_file().path().map(RelativePathBuf::from) {
                    file_events.push(DiffFileEvent {
                        from: r,
                        to: delta.new_file().path().map(RelativePathBuf::from),
                    });
                }
                true
            },
            None,
            None,
            Some(&mut |delta, _, line| {
                if let Some(r) = delta.old_file().path().map(RelativePathBuf::from) {
                    line_events.push(DiffLineEvent {
                        key: r,
                        from: line.old_lineno(),
                        to: line.new_lineno(),
                    });
                }
                true
            }),
        )
        .context(ErrorKind::DiffFailed)?;
        let mut changes = Changes::new();
        file_events
            .into_iter()
            .for_each(|e| changes.process_file(e));
        line_events
            .into_iter()
            .for_each(|e| changes.process_line(e));
        Ok(changes)
    }
}

impl VCS for Git {
    fn get_current_version(&self, repo_path: AbsolutePath<'_>) -> Result<String> {
        let repo = Repository::open(repo_path.as_path())
            .context(ErrorKind::InvalidRepositoryPath)
            .or_else(|e| {
                Err(Error::from(e)
                    .attach("repo_path", format!("{}", repo_path.as_path().display())))
            })?;
        let id = repo
            .head()
            .and_then(|head| Ok(head.peel_to_commit()?.id()))
            .context(ErrorKind::InvalidCommitHash)?;
        Ok(format!("{}", id))
    }

    fn lookup_file_bytes(
        &self,
        repo_path: AbsolutePath<'_>,
        commit: &str,
        file_path: &RelativePathBuf,
    ) -> Result<Vec<u8>> {
        let repo = Repository::open(repo_path.as_path())
            .context(ErrorKind::InvalidRepositoryPath)
            .or_else(|e| {
                Err(Error::from(e)
                    .attach("repo_path", format!("{}", repo_path.as_path().display())))
            })?;
        let rev = format!("{}:{}", commit, file_path.as_git_path());
        let obj = repo
            .revparse_single(&rev)
            .context(ErrorKind::FailedToParseRevision)?;
        let blob = obj.as_blob().ok_or(ErrorKind::FailedToParseRevision)?;
        Ok(blob.content().to_vec())
    }

    fn is_workspace_dirty(&self, repo_path: AbsolutePath<'_>) -> Result<bool> {
        let changes = self.diff(repo_path, "HEAD", None)?;
        Ok(!changes.is_empty())
    }

    fn diff_with_version(
        &self,
        repo_path: AbsolutePath<'_>,
        from: &str,
        to: &str,
    ) -> Result<Changes> {
        self.diff(repo_path, from, Some(to))
    }

    fn diff_with_worktree(&self, repo_path: AbsolutePath<'_>, from: &str) -> Result<Changes> {
        self.diff(repo_path, from, None)
    }
}

#[cfg(test)]
mod tests {
    use super::changes::{FileChanges, LineChanges};
    use super::{Git, VCS};
    use crate::types::path::{AbsolutePathBuf, RelativePathBuf};
    use git2::{Commit, ObjectType, Oid, Repository, Signature};
    use std::fs;
    use std::path::Path;
    use std::str;
    use tempdir::TempDir;

    fn find_last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
        let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
        obj.into_commit()
            .map_err(|_| git2::Error::from_str("Couldn't find commit"))
    }

    fn add_files<P: AsRef<Path>>(repo: &Repository, files: Vec<P>) -> Result<Oid, git2::Error> {
        let mut index = repo.index()?;
        for file in files {
            index.add_path(file.as_ref())?;
        }
        index.write_tree()
    }

    fn commit(repo: &Repository, oid: Oid, message: &str) -> Result<Oid, git2::Error> {
        let signature = Signature::now("Test User", "test@user.net")?;
        let tree = repo.find_tree(oid)?;
        let parent = match find_last_commit(&repo) {
            Ok(p) => vec![p],
            Err(_) => vec![],
        };
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent.iter().map(|x| &*x).collect::<Vec<_>>(),
        )
    }

    #[test]
    fn simple_diffs_work() {
        let repo_dir = TempDir::new("my_repo").unwrap().into_path();
        let repo = Repository::init(&repo_dir).unwrap();

        let file1 = repo_dir.join("test.txt");
        let file2 = repo_dir.join("test2.txt");

        fs::write(&file1, "Hello, world!\nSomething else").unwrap();
        fs::write(&file2, "1\n2\n3").unwrap();

        let oid = add_files(&repo, vec!["test.txt", "test2.txt"]).unwrap();
        let from_id = commit(&repo, oid, "commit 1").unwrap();

        fs::write(&file1, "Poop\nHello, world!\nGoodbye, world!").unwrap();
        fs::write(&file2, "2\n3\n4").unwrap();

        let oid = add_files(&repo, vec!["test.txt", "test2.txt"]).unwrap();
        let to_id = commit(&repo, oid, "commit 2").unwrap();

        let changes = Git
            .diff_with_version(
                AbsolutePathBuf::new(repo_dir.clone())
                    .unwrap()
                    .as_absolute_path(),
                &format!("{:?}", from_id),
                &format!("{:?}", to_id),
            )
            .unwrap();

        assert_eq!(
            Some(&FileChanges::Changed {
                line_changes: LineChanges {
                    changes: vec![(1, 2)].into_iter().collect(),
                    additions: vec![1, 3].into_iter().collect(),
                    deletions: vec![2].into_iter().collect(),
                }
            }),
            changes.for_file(&RelativePathBuf::from(Path::new("test.txt")))
        )
    }
}
