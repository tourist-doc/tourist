use crate::error::{Result, Error};
use crate::resolve;
use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use tourist_types::{Index, Tour};
use zip;

pub fn package_tour(zip_path: &Path, index: Index, tour: Tour, tour_source: &str) -> Result<()> {
    let f = File::create(zip_path)?;
    let mut zip = zip::ZipWriter::new(f);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let mut files = HashSet::new();
    for stop in tour.stops {
        files.insert((stop.repository, stop.path));
    }

    for (repository, path) in files {
        let content = resolve::lookup_file_bytes(
            index
                .get(&repository)
                .ok_or(Error::NotInIndex(repository.clone()))?
                .as_absolute_path(),
            tour.repositories
                .get(&repository)
                .ok_or(Error::NoCommitForRepository(repository.clone()))?,
            &path,
        )?;
        let mut file = PathBuf::from(&repository);
        file.push(path.as_path_buf());

        zip.start_file(
            file.to_str()
                .ok_or(Error::IO(io::Error::from(io::ErrorKind::Other)))?,
            options,
        )?;
        zip.write(&content)?;
    }

    zip.start_file("tour.tour", options)?;
    zip.write(tour_source.as_bytes())?;

    Ok(())
}
