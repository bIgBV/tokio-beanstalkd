use std::fmt::{self, Display};

pub type Tube = String;

pub type Id = u32;

#[derive(Debug, PartialEq, Eq)]
pub struct PreJob {
    pub id: Id,
    pub bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub id: Id,
    pub bytes: usize,
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Response {
    OK,
    Reserved(Job),
    Inserted(Id),
    Buried(Id),
    Using(Tube),
    Deleted,
    Watching,
    NotIgnored,

    ConnectionClosed,
    // Custom type used for reserved job response parsing.
    Pre(PreJob),
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
