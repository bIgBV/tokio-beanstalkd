//! Error types returned by Beanstalkd

use std::io;

use thiserror::Error;

/// Enum that helps understand what kind of a decoder error occurred.
#[derive(Debug, Error)]
pub(crate) enum Decode {
    #[error("A protocol error occurred")]
    Protocol(#[from] ProtocolError),
    #[error("A parsing error occurred")]
    Parsing(#[from] ParsingError),

    #[error("Io error occurred")]
    IoError(#[from] io::Error),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Error)]
pub(crate) enum ParsingError {
    /// Represents errors when parsing an Integer value such as a Job ID or the
    /// number of tubes being watched
    #[error("ID parse error")]
    ParseId,

    /// Represents any errors which occur when converting the parsed ASCII string
    /// to UTF8. This should not occur as the Beanstalkd protocol only works with
    /// ASCII names
    #[error("Stringn parse error")]
    ParseString,

    /// Error occurred while parsing a number
    #[error("Number parse error")]
    ParseNumber,

    /// If some unknown error occurred.
    #[error("Unknown")]
    UnknownResponse,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Error)]
pub(crate) enum ProtocolError {
    #[error("BadFormat")]
    BadFormat,
    #[error("OutOfMemory")]
    OutOfMemory,
    #[error("InternalError")]
    InternalError,
    #[error("UnknownCommand")]
    UnknownCommand,
    #[error("ExpectedCRLF")]
    ExpectedCRLF,
    #[error("JobTooBig")]
    JobTooBig,
    #[error("Draining")]
    Draining,
    #[error("NotFound")]
    NotFound,
    #[error("NotIgnored")]
    NotIgnored,
    #[error("StreamClosed")]
    StreamClosed,
}

/// Custom error type which represents all the various errors which can occur when encoding a
/// request for the server.
#[derive(Debug, Error)]
pub(crate) enum EncodeError {
    #[error("Tube name too long")]
    TubeNameTooLong,

    #[error("IO error")]
    IoError(#[from] io::Error),
}
