//! Error types returned by Beanstalkd

/// Errors that can be returned for any command
#[derive(Copy, Clone, Debug, Eq, PartialEq, Fail)]
pub enum BeanstalkError {
    /// The client sent a command line that was not well-formed. This can happen if the line does not
    /// end with \r\n, if non-numeric characters occur where an integer is expected, if the wrong
    /// number of arguments are present, or if the command line is mal-formed in any other way.
    ///
    /// This should not happen, if it does please file an issue.
    #[fail(display = "Client command was not well formatted")]
    BadFormat,

    /// The server cannot allocate enough memory for the job. The client should try again later.
    #[fail(display = "Server out of memory")]
    OutOfMemory,

    /// This indicates a bug in the server. It should never happen. If it does happen, please report it
    /// at http://groups.google.com/group/beanstalk-talk.
    #[fail(display = "Internal server error")]
    InternalError,
    /// The client sent a command that the server does not know.
    ///
    /// This should not happen, if it does please file an issue.
    #[fail(display = "Unknown command sent by client")]
    UnknownCommand,
}

/// Errors which can be casued due to a PUT command
#[derive(Copy, Clone, Debug, Eq, PartialEq, Fail)]
pub enum Put {
    /// The server ran out of memory trying to grow the priority queue data structure.
    /// The client should try another server or disconnect and try again later.
    #[fail(display = "Server had to bury the request")]
    Buried,

    /// The job body must be followed by a CR-LF pair, that is, "\r\n". These two bytes are not counted
    /// in the job size given by the client in the put command line.
    ///
    /// This should never happen, if it does please file an issue.
    #[fail(display = "CRLF missing from the end of command")]
    ExpectedCLRF,

    /// The client has requested to put a job with a body larger than max-job-size bytes
    #[fail(display = "Job size exceeds max-job-size bytes")]
    JobTooBig,

    /// This means that the server has been put into "drain mode" and is no longer accepting new jobs.
    /// The client should try another server or disconnect and try again later.
    #[fail(display = "Server is in drain mode")]
    Draining,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Fail)]
pub enum Consumer {
    /// If the job does not exist or is not either reserved by the client
    #[fail(display = "Did not find a job of that Id")]
    NotFound,
    /// if the server ran out of memory trying to grow the priority queue data structure.
    #[fail(display = "Job got buried")]
    Buried,
    /// If the client attempts to ignore the only tube in its watch list.
    #[fail(display = "Tried to ignore the only tube being watched")]
    NotIgnored,
}
