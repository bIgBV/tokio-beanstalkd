//! Response types returned by Beanstalkd
use std::fmt::{self, Display};

/// This is an internal type which is not returned by tokio-beanstalkd.
/// It is used when parsing the job data returned by Beanstald.
#[derive(Debug, PartialEq, Eq)]
pub struct PreJob {
    pub id: super::Id,
    pub bytes: usize,
}

/// A job according to Beanstalkd
#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub id: super::Id,
    pub bytes: usize,
    pub data: Vec<u8>,
}

/// The response from Beanstalkd
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
    Found(Job),

    ConnectionClosed,
    // Custom type used for reserved job response parsing.
    Pre(PreJob),
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
