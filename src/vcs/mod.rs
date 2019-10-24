use crate::error::{Error, ErrorKind, Result};
use crate::types::path::{AbsolutePath, RelativePathBuf};
use failure::ResultExt;
use git2::{Commit, DiffOptions, ObjectType, Oid, Repository};

mod changes;

pub use changes::{Changes, FileChanges, LineChanges};
use changes::{DiffFileEvent, DiffLineEvent};

pub trait VCS {
    fn get_current_version(&self, repo_path: AbsolutePath<'_>) -> Result<String>;

    fn diff_with_version(
        &self,
        repo_path: AbsolutePath<'_>,
        from: &str,
        to: &str,
    ) -> Result<Changes>;

    fn diff_with_worktree(&self, repo_path: AbsolutePath<'_>, from: &str) -> Result<Changes>;

    fn is_workspace_dirty(&self, repo_path: AbsolutePath<'_>) -> Result<bool>;

    fn checkout_version(&self, repo_path: AbsolutePath<'_>, to: &str) -> Result<String>;

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
        let from_oid = Oid::from_str(from).context(ErrorKind::InvalidCommitHash)?;
        let to_oid = match to {
            Some(to) => Some(Oid::from_str(to).context(ErrorKind::InvalidCommitHash)?),
            None => None,
        };
        self.diff_oid(repo_path, from_oid, to_oid)
    }

    fn diff_oid(&self, repo_path: AbsolutePath<'_>, from: Oid, to: Option<Oid>) -> Result<Changes> {
        let repo = Repository::open(repo_path.as_path())
            .context(ErrorKind::InvalidRepositoryPath)
            .or_else(|e| {
                Err(Error::from(e)
                    .attach("repo_path", format!("{}", repo_path.as_path().display())))
            })?;

        let from_tree = repo
            .find_commit(from)
            .and_then(|c| c.tree())
            .context(ErrorKind::InvalidCommitHash)?;
        let mut opts = DiffOptions::new();
        opts.minimal(true);
        opts.ignore_whitespace_eol(true);

        let diff = if let Some(to) = to {
            let to_tree = repo
                .find_commit(to)
                .and_then(|c| c.tree())
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

    fn head_commit<'a>(&self, repo: &'a Repository) -> Result<Commit<'a>> {
        let obj = repo
            .head()
            .and_then(|h| h.resolve())
            .and_then(|o| o.peel(ObjectType::Commit))
            .context(ErrorKind::InvalidCommitHash)?;
        let commit = obj
            .into_commit()
            .map_err(|_| git2::Error::from_str("object not commit"))
            .context(ErrorKind::InvalidCommitHash)?;
        Ok(commit)
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
        let id = self.head_commit(&repo)?.id();
        Ok(format!("{}", id))
    }

    fn lookup_file_bytes(
        &self,
        repo_path: AbsolutePath<'_>,
        commit: &str,
        file_path: &RelativePathBuf,
    ) -> Result<Vec<u8>> {
        let repo = Repository::open(repo_path.as_path()).map_err(|_| {
            ErrorKind::InvalidRepositoryPath
                .attach("repo_path", format!("{}", repo_path.as_path().display()))
        })?;

        let rev = format!("{}:{}", commit, file_path.as_git_path());
        let obj = repo
            .revparse_single(&rev)
            .context(ErrorKind::FailedToParseRevision)?;
        let blob = obj.as_blob().ok_or(ErrorKind::FailedToParseRevision)?;
        Ok(blob.content().to_vec())
    }

    fn is_workspace_dirty(&self, repo_path: AbsolutePath<'_>) -> Result<bool> {
        let repo = Repository::open(repo_path.as_path()).map_err(|_| {
            ErrorKind::InvalidRepositoryPath
                .attach("repo_path", format!("{}", repo_path.as_path().display()))
        })?;
        let commit = self.head_commit(&repo)?;
        let changes = self.diff_oid(repo_path, commit.id(), None)?;
        Ok(!changes.is_empty())
    }

    fn checkout_version(&self, repo_path: AbsolutePath<'_>, to: &str) -> Result<String> {
        if self.is_workspace_dirty(repo_path)? {
            return Err(ErrorKind::WorkspaceIsDirty.into());
        }
        let old_version = self.get_current_version(repo_path)?;
        let repo = Repository::open(repo_path.as_path()).map_err(|_| {
            ErrorKind::InvalidRepositoryPath
                .attach("repo_path", format!("{}", repo_path.as_path().display()))
        })?;
        let oid = Oid::from_str(to).context(ErrorKind::InvalidCommitHash)?;
        let obj = repo
            .find_object(oid, Some(ObjectType::Commit))
            .context(ErrorKind::InvalidCommitHash)?;
        repo.checkout_tree(&obj, None)
            .context(ErrorKind::FailedToCheckOutRepository)?;
        repo.set_head_detached(oid)
            .context(ErrorKind::FailedToCheckOutRepository)?;
        Ok(old_version)
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
    use git2::{Oid, Repository, Signature};
    use std::fs;
    use std::path::Path;
    use std::str;
    use tempdir::TempDir;

    fn add_all(repo: &Repository) -> Result<Oid, git2::Error> {
        let mut index = repo.index()?;
        index.add_all(
            vec![] as Vec<String>,
            git2::IndexAddOption::CHECK_PATHSPEC,
            None,
        )?;
        index.write()?;
        index.write_tree_to(repo)
    }

    fn commit(repo: &Repository, oid: Oid, message: &str) -> Result<Oid, git2::Error> {
        let signature = Signature::now("Test User", "test@user.net")?;
        let tree = repo.find_tree(oid)?;
        let parent = match Git.head_commit(&repo) {
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
        let repo_dir = TempDir::new("my_repo").expect("TempDir fail").into_path();
        let repo = Repository::init(&repo_dir).expect("could not init repo");

        let file1 = repo_dir.join("test.txt");
        let file2 = repo_dir.join("test2.txt");

        fs::write(&file1, "Hello, world!\nSomething else").expect("write fail");
        fs::write(&file2, "1\n2\n3").expect("write fail");

        let oid = add_all(&repo).expect("add fail");
        let from_id = commit(&repo, oid, "commit 1").expect("commit fail");

        fs::write(&file1, "Poop\nHello, world!\nGoodbye, world!").expect("write fail");
        fs::write(&file2, "2\n3\n4").expect("write fail");

        let oid = add_all(&repo).expect("add fail");
        let to_id = commit(&repo, oid, "commit 2").expect("commit fail");

        let changes = Git
            .diff_with_version(
                AbsolutePathBuf::new(repo_dir.clone())
                    .expect("simple_diffs_work crash")
                    .as_absolute_path(),
                &format!("{:?}", from_id),
                &format!("{:?}", to_id),
            )
            .expect("diff failed");

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

    fn oid_to_string(oid: Oid) -> String {
        oid.as_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }

    #[test]
    fn can_check_out() {
        let repo_dir = TempDir::new("my_repo").expect("TempDir fail").into_path();
        let repo = Repository::init(&repo_dir).expect("repo init fail");

        dbg!(&repo_dir);

        let file = repo_dir.join("test.txt");

        fs::write(&file, "Hello, world!").expect("write fail");

        let oid = add_all(&repo).expect("add fail");
        let first_id = commit(&repo, oid, "commit 1").expect("commit fail");

        fs::write(&file, "Goodbye world!").expect("write fail");

        let oid = add_all(&repo).expect("add fail");
        let _ = commit(&repo, oid, "commit 2").expect("commit fail");

        Git.checkout_version(
            AbsolutePathBuf::new(repo_dir.clone())
                .expect("path not absolute")
                .as_absolute_path(),
            &oid_to_string(first_id),
        )
        .expect("checkout failed for some reason");

        assert_eq!(
            fs::read_to_string(&file).expect("read fail"),
            "Hello, world!"
        );
    }
}
