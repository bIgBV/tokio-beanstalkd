//! The errors returned by the different operations in the library
use thiserror::Error;

use crate::proto::error::{Decode, EncodeError, ProtocolError};

/// Errors that can be returned for any command
#[derive(Copy, Clone, Debug, Eq, PartialEq, Error)]
pub enum BeanstalkError {
    /// The client sent a command line that was not well-formed. This can happen if the line does not
    /// end with \r\n, if non-numeric characters occur where an integer is expected, if the wrong
    /// number of arguments are present, or if the command line is mal-formed in any other way.
    ///
    /// This should not happen, if it does please file an issue.
    #[error("Client command was not well formatted")]
    BadFormat,

    /// The server cannot allocate enough memory for the job. The client should try again later.
    #[error("Server out of memory")]
    OutOfMemory,

    /// This indicates a bug in the server. It should never happen. If it does happen, please report it
    /// at http://groups.google.com/group/beanstalk-talk.
    #[error("Internal server error")]
    InternalError,
    /// The client sent a command that the server does not know.
    ///
    /// This should not happen, if it does please file an issue.
    #[error("Unknown command sent by client")]
    UnknownCommand,

    #[error("An unexpected response occurred")]
    UnexpectedResponse,

    #[error("Tube name too long")]
    TubeNameTooLong,

    #[error("IO Error")]
    IoError,

    #[doc(hidden)]
    #[error("Just an extention..")]
    __Nonexhaustive,
}

impl From<Decode> for BeanstalkError {
    fn from(error: Decode) -> Self {
        match error {
            Decode::Protocol(ProtocolError::BadFormat) => BeanstalkError::BadFormat,
            Decode::Protocol(ProtocolError::OutOfMemory) => BeanstalkError::OutOfMemory,
            Decode::Protocol(ProtocolError::InternalError) => BeanstalkError::InternalError,
            Decode::Protocol(ProtocolError::UnknownCommand) => BeanstalkError::UnknownCommand,
            _ => BeanstalkError::UnexpectedResponse,
        }
    }
}

impl From<EncodeError> for BeanstalkError {
    fn from(error: EncodeError) -> Self {
        match error {
            EncodeError::TubeNameTooLong => BeanstalkError::TubeNameTooLong,
            EncodeError::IoError(_) => BeanstalkError::IoError,
        }
    }
}

/// Errors which can be casued due to a PUT command
#[derive(Copy, Clone, Debug, Eq, PartialEq, Error)]
pub enum Put {
    /// The server ran out of memory trying to grow the priority queue data structure.
    /// The client should try another server or disconnect and try again later.
    #[error("Server had to bury the request")]
    Buried,

    /// The job body must be followed by a CR-LF pair, that is, "\r\n". These two bytes are not counted
    /// in the job size given by the client in the put command line.
    ///
    /// This should never happen, if it does please file an issue.
    #[error("CRLF missing from the end of command")]
    ExpectedCRLF,

    /// The client has requested to put a job with a body larger than max-job-size bytes
    #[error("Job size exceeds max-job-size bytes")]
    JobTooBig,

    /// This means that the server has been put into "drain mode" and is no longer accepting new jobs.
    /// The client should try another server or disconnect and try again later.
    #[error("Server is in drain mode")]
    Draining,

    #[error("A protocol error occurred: {}", error)]
    Beanstalk { error: BeanstalkError },

    #[doc(hidden)]
    #[error("Just an extention..")]
    __Nonexhaustive,
}

impl From<Decode> for Put {
    fn from(error: Decode) -> Self {
        match error {
            Decode::Protocol(ProtocolError::ExpectedCRLF) => Put::ExpectedCRLF,
            Decode::Protocol(ProtocolError::JobTooBig) => Put::JobTooBig,
            Decode::Protocol(ProtocolError::Draining) => Put::Draining,
            _ => Put::Beanstalk {
                error: error.into(),
            },
        }
    }
}

impl From<EncodeError> for Put {
    fn from(error: EncodeError) -> Self {
        Put::Beanstalk {
            error: error.into(),
        }
    }
}

/// Errors which can occur when acting as a consumer/worker
#[derive(Copy, Clone, Debug, Eq, PartialEq, Error)]
pub enum Consumer {
    /// If the job does not exist or is not either reserved by the client
    #[error("Did not find a job of that Id")]
    NotFound,
    /// if the server ran out of memory trying to grow the priority queue data structure.
    #[error("Job got buried")]
    Buried,
    /// If the client attempts to ignore the only tube in its watch list.
    #[error("Tried to ignore the only tube being watched")]
    NotIgnored,

    #[error("A protocol error occurred: {}", error)]
    Beanstalk { error: BeanstalkError },

    #[doc(hidden)]
    #[error("Just an extention..")]
    __Nonexhaustive,
}

impl From<Decode> for Consumer {
    fn from(error: Decode) -> Self {
        match error {
            Decode::Protocol(ProtocolError::NotFound) => Consumer::NotFound,
            Decode::Protocol(ProtocolError::NotIgnored) => Consumer::NotIgnored,
            _ => Consumer::Beanstalk {
                error: error.into(),
            },
        }
    }
}

impl From<EncodeError> for Consumer {
    fn from(error: EncodeError) -> Self {
        Consumer::Beanstalk {
            error: error.into(),
        }
    }
}
