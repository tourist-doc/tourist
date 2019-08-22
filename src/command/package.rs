use crate::error::{ErrorKind, Result};
use crate::types::{Index, Tour};
use crate::vcs::VCS;
use failure::ResultExt;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip;

pub struct Package<V: VCS, I: Index> {
    vcs: V,
    index: I,
}

impl<V: VCS, I: Index> Package<V, I> {
    pub fn new(vcs: V, index: I) -> Self {
        Package { vcs, index }
    }

    pub fn process(&self, zip_path: &Path, tour: Tour, tour_source: &str) -> Result<()> {
        let f = File::create(zip_path).context(ErrorKind::FailedToWriteZip)?;
        let mut zip = zip::ZipWriter::new(f);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        let mut files = HashSet::new();
        for stop in tour.stops {
            files.insert((stop.repository, stop.path));
        }

        for (repository, path) in files {
            let content = self.vcs.lookup_file_bytes(
                self.index
                    .get(&repository)
                    .ok_or(ErrorKind::RepositoryNotInIndex)?
                    .as_absolute_path(),
                tour.repositories
                    .get(&repository)
                    .ok_or(ErrorKind::NoVersionForRepository)?,
                &path,
            )?;
            let mut file = PathBuf::from(&repository);
            file.push(path.as_path_buf());

            zip.start_file(file.to_str().ok_or(ErrorKind::EncodingFailure)?, options)
                .context(ErrorKind::ZipFailure)?;
            let _ = zip.write(&content).context(ErrorKind::FailedToWriteZip)?;
        }

        zip.start_file("tour.tour", options)
            .context(ErrorKind::ZipFailure)?;
        let _ = zip
            .write(tour_source.as_bytes())
            .context(ErrorKind::FailedToWriteZip)?;

        Ok(())
    }
}
