//! Error types returned by Beanstalkd
use failure::{Backtrace, Context, Fail};
use std::fmt;
use std::fmt::Display;
use std::io;

#[derive(Debug)]
pub(crate) struct Decode {
    inner: Context<ErrorKind>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub(crate) enum ErrorKind {
    #[fail(display = "A protocol error occurred")]
    Protocol(ProtocolError),
    #[fail(display = "A parsing error occurred")]
    Parsing(ParsingError),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum ParsingError {
    ParseId,
    ParseString,
    UnknownResponse,
}

impl Fail for Decode {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Decode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

// Why do I have to implement this?
impl From<io::Error> for Decode {
    fn from(kind: io::Error) -> Decode {
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
    Buried,
    ExpectedCRLF,
    JobTooBig,
    Draining,
    NotFound,
    NotIgnored,
}
