//! Response types returned by Beanstalkd
use std::fmt::{self, Display};

#[derive(Debug, PartialEq, Eq)]
pub struct PreJob {
    pub id: super::Id,
    pub bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub id: super::Id,
    pub bytes: usize,
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Response {
    OK,
    Reserved(Job),
    Inserted(super::Id),
    Buried,
    Using(super::Tube),
    Deleted,
    Watching(u32),
    NotIgnored,
    Released,
    Touched,

    ConnectionClosed,
    // Custom type used for reserved job response parsing.
    Pre(PreJob),
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
