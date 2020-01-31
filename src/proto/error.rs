//! Error types returned by Beanstalkd
use failure::{Backtrace, Context, Fail};
use std::fmt;
use std::fmt::Display;
use std::io;

/// Custom error type which represents all the various errors which can occur
/// when decoding a response from the server
#[derive(Debug)]
pub(crate) struct Decode {
    inner: Context<ErrorKind>,
}

/// Enum that helps understand what kind of a decoder error occurred.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub(crate) enum ErrorKind {
    #[fail(display = "A protocol error occurred")]
    Protocol(ProtocolError),
    #[fail(display = "A parsing error occurred")]
    Parsing(ParsingError),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum ParsingError {
    /// Represents errors when parsing an Integer value such as a Job ID or the
    /// number of tubes being watched
    ParseId,

    /// Represents any errors which occur when converting the parsed ASCII string
    /// to UTF8. This should not occur as the Beanstalkd protocol only works with
    /// ASCII names
    ParseString,

    /// Error occurred while parsing a number
    ParseNumber,

    /// If some unknown error occurred.
    UnknownResponse,
}

/// This helps us keep track of the underlying cause of the error
impl Fail for Decode {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Decode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Decode {
    pub fn kind(&self) -> ErrorKind {
        *self.inner.get_context()
    }
}

impl From<ErrorKind> for Decode {
    fn from(kind: ErrorKind) -> Decode {
        Decode {
            inner: Context::new(kind),
        }
    }
}

// From the tokio_codec::Encoder trait docs:
//
//      > FramedWrite requires Encoders errors to implement From<io::Error> in the interest letting
//      > it return Errors directly.
impl From<io::Error> for Decode {
    fn from(_kind: io::Error) -> Decode {
        Decode {
            inner: Context::new(ErrorKind::Parsing(ParsingError::ParseString)),
        }
    }
}

impl From<Context<ErrorKind>> for Decode {
    fn from(inner: Context<ErrorKind>) -> Decode {
        Decode { inner }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum ProtocolError {
    BadFormat,
    OutOfMemory,
    InternalError,
    UnknownCommand,
    ExpectedCRLF,
    JobTooBig,
    Draining,
    NotFound,
    NotIgnored,
    StreamClosed,
}

/// Custom error type which represents all the various errors which can occur when encoding a
/// request for the server.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub(crate) enum EncodeError {
    #[fail(display = "Tube name too long")]
    TubeNameTooLong,

    #[fail(display = "IO error")]
    IoError,
}

// From the tokio_codec::Encoder trait docs:
//
//      > FramedWrite requires Encoders errors to implement From<io::Error> in the interest letting
//      > it return Errors directly.
impl From<io::Error> for EncodeError {
    fn from(_error: io::Error) -> Self {
        EncodeError::IoError
    }
}
