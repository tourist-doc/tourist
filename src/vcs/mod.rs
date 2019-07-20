use crate::error::{Error, Result};
use crate::types::path::{AbsolutePath, RelativePathBuf};
use git2::{DiffOptions, Oid, Repository};

mod changes;

pub use changes::Changes;
use changes::{DiffFileEvent, DiffLineEvent};

pub trait VCS {
    fn lookup_file_bytes(
        &self,
        repo_path: AbsolutePath<'_>,
        commit: &str,
        file_path: &RelativePathBuf,
    ) -> Result<Vec<u8>>;

    fn diff_with_version(
        &self,
        repo_path: AbsolutePath<'_>,
        from: &str,
        to: &str,
    ) -> Result<Changes>;

    fn diff_with_worktree(&self, repo_path: AbsolutePath<'_>, from: &str) -> Result<Changes>;

    fn lookup_file_contents(
        &self,
        repo_path: AbsolutePath<'_>,
        commit: &str,
        file_path: &RelativePathBuf,
    ) -> Result<String> {
        let content = self.lookup_file_bytes(repo_path, commit, file_path)?;
        Ok(std::str::from_utf8(&content)?.to_owned())
    }
}

pub struct Git;

impl VCS for Git {
    fn lookup_file_bytes(
        &self,
        repo_path: AbsolutePath<'_>,
        commit: &str,
        file_path: &RelativePathBuf,
    ) -> Result<Vec<u8>> {
        let repo = Repository::open(repo_path.as_path())?;
        let rev = format!("{}:{}", commit, file_path.as_git_path());
        let obj = repo.revparse_single(&rev)?;
        let blob = obj.as_blob().ok_or(Error::RevParse(rev))?;
        Ok(blob.content().to_vec())
    }

    fn diff_with_version(
        &self,
        repo_path: AbsolutePath<'_>,
        from: &str,
        to: &str,
    ) -> Result<Changes> {
        let repo = Repository::open(repo_path.as_path())?;
        let from_tree = repo.find_commit(Oid::from_str(from)?)?.tree()?;
        let to_tree = repo.find_commit(Oid::from_str(to)?)?.tree()?;

        let diff = repo.diff_tree_to_tree(
            Some(&from_tree),
            Some(&to_tree),
            Some(&mut DiffOptions::new().minimal(true).ignore_whitespace_eol(true)),
        )?;

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
        )?;
        let mut changes = Changes::new();
        file_events
            .into_iter()
            .for_each(|e| changes.process_file(e));
        line_events
            .into_iter()
            .for_each(|e| changes.process_line(e));
        Ok(changes)
    }

    fn diff_with_worktree(&self, _repo_path: AbsolutePath<'_>, _from: &str) -> Result<Changes> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::changes::FileChanges;
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
    fn figure_out_diff() {
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
                deletions: vec![2].into_iter().collect(),
                changes: vec![(1, 2)].into_iter().collect(),
            }),
            changes.for_file(&RelativePathBuf::from(Path::new("test.txt")))
        )
    }
}
