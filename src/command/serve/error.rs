use failure::{Backtrace, Context, Fail};
use jsonrpc_core::Result as JsonResult;
use std::fmt;
use std::fmt::Display;

pub trait AsJsonResult<T> {
    fn as_json_result(self) -> JsonResult<T>;
}

impl<T, F: Fail> AsJsonResult<T> for Result<T, F> {
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
}

#[derive(Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "no tour is currently being tracked with ID '{}'", id)]
    NoTourFound { id: String },
    #[fail(
        display = "tour with ID '{}' has no stop with ID '{}'",
        tour_id, stop_id
    )]
    NoStopFound { tour_id: String, stop_id: String },
    #[fail(display = "the given path '{}' is not absolute", path)]
    ExpectedAbsolutePath { path: String },
    #[fail(
        display = "the path '{}' does not appear to be in a Git repository",
        path
    )]
    PathNotInIndex { path: String },
    #[fail(display = "could not read the provided tour file")]
    FailedToReadTour,
    #[fail(display = "could not parse the provided tour file")]
    FailedToParseTour,
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
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}
