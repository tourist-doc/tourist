use failure::{Backtrace, Context, Fail};
use jsonrpc_core::Result as JsonResult;
use slog_scope::error;
use std::fmt;
use std::fmt::Display;

pub type Result<T> = std::result::Result<T, Error>;

pub trait AsJsonResult<T> {
    fn as_json_result(self) -> JsonResult<T>;
}

impl<T> AsJsonResult<T> for std::result::Result<T, Error> {
    fn as_json_result(self) -> JsonResult<T> {
        self.or_else(|e| {
            error!("JSON Result Error: {}", e);
            let mut err = jsonrpc_core::Error::internal_error();
            err.data = Some(format!("{}", e).into());
            err.code = jsonrpc_core::ErrorCode::ServerError(error_code(e.inner.get_context()));
            Err(err)
        })
    }
}

pub fn error_code(kind: &ErrorKind) -> i64 {
    use ErrorKind::*;
    match kind {
        // Recoverable
        NoRepositoryForFile => 300,
        RepositoryNotInIndex => 301,
        TourNotEditable => 310,
        TourNotUpToDate => 311,
        NoPathForTour => 320,

        // Input Errors
        NoTourWithID => 400,
        NoStopWithID => 401,

        // Config Errors
        InvalidRepositoryPath => 410,
        ExpectedAbsolutePath => 411,

        // Tour File Inconsistencies
        InvalidCommitHash => 420,
        NoVersionForRepository => 421,

        // IO Errors
        FailedToReadTour => 500,
        FailedToWriteTour => 501,
        FailedToReadIndex => 510,
        FailedToWriteIndex => 511,
        FailedToWriteZip => 520,
        FailedToSerializeTour => 530,
        FailedToSerializeIndex => 531,
        FailedToParseTour => 541,
        FailedToParseRevision => 542,
        FailedToParseIndex => 543,

        // Anomalies
        EncodingFailure => 600,
        ZipFailure => 601,
        PositionDeltaOutOfRange => 602,
        DiffFailed => 603,
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
    NoTourWithID,
    #[fail(display = "could not find the specified stop")]
    NoStopWithID,
    #[fail(display = "no git repository exists at the provided path")]
    InvalidRepositoryPath,
    #[fail(display = "internal error: git failed to process commit")]
    InvalidCommitHash,
    #[fail(display = "tour has not been saved and does not have a path")]
    NoPathForTour,
    #[fail(display = "no version for repsoitory")]
    NoVersionForRepository,
    #[fail(display = "the provided path was not absolute")]
    ExpectedAbsolutePath,
    #[fail(display = "file path is not in an indexed git repository")]
    NoRepositoryForFile,
    #[fail(display = "repsoitory does not appear to be mapped to a git repository")]
    RepositoryNotInIndex,
    #[fail(display = "could not read the provided tour file")]
    FailedToReadTour,
    #[fail(display = "could not read repository index")]
    FailedToReadIndex,
    #[fail(display = "could not write the provided tour file")]
    FailedToWriteTour,
    #[fail(display = "could not write repository index")]
    FailedToWriteIndex,
    #[fail(display = "could not write zip package")]
    FailedToWriteZip,
    #[fail(display = "could not parse the provided tour file")]
    FailedToParseTour,
    #[fail(display = "could not serialize the provided tour file")]
    FailedToSerializeTour,
    #[fail(display = "failed to process a git diff")]
    DiffFailed,
    #[fail(display = "failed to parse the saved git revision")]
    FailedToParseRevision,
    #[fail(display = "failed to parse the repository index")]
    FailedToParseIndex,
    #[fail(display = "failed to serialize repository index")]
    FailedToSerializeIndex,
    #[fail(display = "file was not saved in UTF-8")]
    EncodingFailure,
    #[fail(display = "something went wrong while creating zip file")]
    ZipFailure,
    #[fail(display = "please open tour as editable to make changes")]
    TourNotEditable,
    #[fail(display = "please stash your local changes and check out correct tour versions")]
    TourNotUpToDate,
    #[fail(display = "position delta was not in the appropriate range")]
    PositionDeltaOutOfRange,
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
