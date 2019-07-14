use crate::error::{Result, Error};
use git2::Repository;
use tourist_types::path::{AbsolutePath, RelativePathBuf};

pub fn lookup_file_bytes(
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

pub fn lookup_file_contents(
    repo_path: AbsolutePath<'_>,
    commit: &str,
    file_path: &RelativePathBuf,
) -> Result<String> {
    let content = lookup_file_bytes(repo_path, commit, file_path)?;
    Ok(std::str::from_utf8(&content)?.to_owned())
}
