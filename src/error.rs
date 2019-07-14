use std::error;
use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Git2(git2::Error),
    Utf8(std::str::Utf8Error),
    IO(io::Error),
    Serde(serde_json::Error),
    Zip(zip::result::ZipError),
    NotInIndex(String),
    NoCommitForRepository(String),
    RevParse(String),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            IO(e) => Some(e),
            Serde(e) => Some(e),
            Git2(e) => Some(e),
            Utf8(e) => Some(e),
            Zip(e) => Some(e),
            NotInIndex(_) => None,
            NoCommitForRepository(_) => None,
            RevParse(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            IO(e) => e.fmt(f),
            Serde(e) => e.fmt(f),
            Git2(e) => e.fmt(f),
            Utf8(e) => e.fmt(f),
            Zip(e) => e.fmt(f),
            NotInIndex(s) => write!(f, "Could not find repository '{}' in index.", s),
            NoCommitForRepository(s) => write!(f, "Could not find commit for repository '{}'.", s),
            RevParse(rev) => write!(f, "Reference '{}' does not point to a blob.", rev),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Error {
        Error::Zip(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::Serde(e)
    }
}

impl From<git2::Error> for Error {
    fn from(e: git2::Error) -> Error {
        Error::Git2(e)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Error {
        Error::Utf8(e)
    }
}
