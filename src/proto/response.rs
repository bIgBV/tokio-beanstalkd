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
    /// The ID job assigned by Beanstalkd
    pub id: super::Id,

    /// The size of the payload
    pub bytes: usize,

    /// The payload
    pub data: Vec<u8>,
}

/// All possible responses that the Beanstalkd server can send.
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
