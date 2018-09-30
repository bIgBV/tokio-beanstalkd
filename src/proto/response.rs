//! Response types returned by Beanstalkd

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

/// A response to an IGNORE command.
#[derive(Debug, PartialEq, Eq)]
pub enum IgnoreResponse {
    /// The integer number of tubes currently left in the watch list.
    Watching(u32),
    /// The client attempted to ignore the only tube in its watch list.
    NotIgnored,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AnyResponse {
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
