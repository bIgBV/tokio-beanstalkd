//! Response types returned by Beanstalkd

/// [pre]: [tokio_beanstalkd::proto::response::PreJob]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PreResponse {
    /// A response for a reserve request
    Reserved,

    /// All types of peek requests have the same response
    Peek,
}

/// This is an internal type which is not returned by tokio-beanstalkd.
/// It is used when parsing the job data returned by Beanstald.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct PreJob {
    pub id: super::Id,
    pub bytes: usize,
    pub response_type: PreResponse,
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

impl PreJob {
    /// Simple method to match a given PreJob to the right Response
    pub(crate) fn to_anyresponse(self, job: Job) -> AnyResponse {
        match self.response_type {
            PreResponse::Reserved => AnyResponse::Reserved(job),
            PreResponse::Peek => AnyResponse::Found(job),
        }
    }
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
