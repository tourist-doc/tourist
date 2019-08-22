use failure::{Backtrace, Context, Fail};
use jsonrpc_core::Result as JsonResult;
use std::fmt;
use std::fmt::Display;

pub type Result<T> = std::result::Result<T, Error>;

pub trait AsJsonResult<T> {
    fn as_json_result(self) -> JsonResult<T>;
}

impl<T, F: Fail> AsJsonResult<T> for std::result::Result<T, F> {
    fn as_json_result(self) -> JsonResult<T> {
        self.or_else(|e| {
            let mut err = jsonrpc_core::Error::internal_error();
            err.data = Some(format!("{}", e).into());
            Err(err)
        })
    }
}

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
    attachments: Vec<String>,
}

#[derive(Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "could not find the specified tour")]
    NoTourFound,
    #[fail(display = "could not find the specified stop")]
    NoStopFound,
    #[fail(display = "tour has not been saved and does not have a path")]
    NoPathForTour,
    #[fail(display = "no version for repsoitory")]
    NoVersionForRepository,
    #[fail(display = "the provided path was not absolute")]
    ExpectedAbsolutePath,
    #[fail(display = "path does not appear to be in a Git repository")]
    PathNotInIndex,
    #[fail(display = "repsoitory does not appear to be mapped to a git repository")]
    RepositoryNotInIndex,
    #[fail(display = "could not read the provided tour file")]
    FailedToReadTour,
    #[fail(display = "could not write the provided tour file")]
    FailedToWriteTour,
    #[fail(display = "could not parse the provided tour file")]
    FailedToParseTour,
    #[fail(display = "could not serialize the provided tour file")]
    FailedToSerializeTour,
    #[fail(display = "failed to process a git diff")]
    DiffFailed,
}

impl Error {
    pub fn attach<K: Display, V: Display>(mut self, k: K, v: V) -> Self {
        self.attachments.push(format!("{}: {}", k, v));
        self
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
            attachments: vec![],
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error {
            inner,
            attachments: vec![],
        }
    }
}

impl ErrorKind {
    pub fn attach<K: Display, V: Display>(self, k: K, v: V) -> Error {
        Error::from(self).attach(k, v)
    }
}
