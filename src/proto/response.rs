use std::fmt::{self, Display};

pub type Tube = String;

pub type Id = u32;

#[derive(Debug)]
pub enum Response {
    OK,
    Reserved,
    Inserted(Id),
    Buried(Id),
    Using(Tube),
    Deleted,
    Watching,
    NotIgnored,
    BadFormat,
    ExpectedCLRF,
    JobTooBig,
    Draining,
    OutOfMemory,
    InternalError,
    UnknownCommand,

    ConnectionClosed,
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
