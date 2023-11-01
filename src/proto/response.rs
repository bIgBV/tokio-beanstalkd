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

pub struct Stats {}

impl PreJob {
    /// Simple method to match a given PreJob to the right Response
    pub(crate) fn to_anyresponse(self, job: Job) -> Response {
        match self.response_type {
            PreResponse::Reserved => Response::Reserved(job),
            PreResponse::Peek => Response::Found(job),
        }
    }
}

/// All possible responses that the Beanstalkd server can send.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Response {
    Reserved(Job),
    Inserted(super::Id),

    /// Specifies buried state for different commands
    ///
    /// - Release: If the server ran out of memory trying to grow the priority queue data structure.
    /// - Bury: to indicate success
    /// -
    Buried(Option<super::Id>),

    /// Response from [`use`][super::Beanstalkd::use]
    ///
    /// - <tube> is the name of the tube now being used.
    Using(super::Tube),


    Deleted,
    Watching(u32),

    /// To indicate success for the `release` command.
    Released,

    Touched,
    Found(Job),
    Kicked(u32),
    JobKicked,

    /// If the job does not exist or is not reserved by the client
    NotFound,

    // Custom type used for reserved job response parsing.
    Pre(PreJob),
}
