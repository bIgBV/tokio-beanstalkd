//! Error types returned by Beanstalkd
use failure::{Context, Fail, Backtrace};
use std::fmt;
use std::fmt::{Display};


#[derive(Debug)]
pub(crate) struct DecoderError {
    inner: Context<ErrorKind>,
}

#[derive(Copy,Clone,Eq,PartialEq,Debug, Fail)]
pub(crate) enum ErrorKind {
    #[fail(display = "A protocol error occurred")]
    Protocol(ProtocolError),
    #[fail(display = "A parsing error occurred")]
    Parsing(ParsingError),
}

#[derive(Copy,Clone,Eq,PartialEq,Debug)]
pub(crate) enum ParsingError {
    ParseId,
    ParseString,
    UnknownResponse
}

impl Fail for DecoderError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl DecoderError {
    pub fn kind(&self) -> ErrorKind {
        *self.inner.get_context()
    }
}

impl From<ErrorKind> for DecoderError {
    fn from(kind: ErrorKind) -> DecoderError {
        DecoderError { inner: Context::new(kind)}
    }
}

impl From<Context<ErrorKind>> for DecoderError {
    fn from(inner: Context<ErrorKind>) -> DecoderError {
        DecoderError { inner }
    }
}

#[derive(Copy,Clone,Eq,PartialEq,Debug)]
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
    NotIgnored
}