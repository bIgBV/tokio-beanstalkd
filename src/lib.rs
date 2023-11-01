//! This crate provides a client for working with [Beanstalkd](https://beanstalkd.github.io/), a simple
//! fast work queue.
//
//! # About Beanstalkd
//!
//! Beanstalkd is a simple fast work queue. It works at the TCP connection level, considering each TCP
//! connection individually. A worker may have multiple connections to the Beanstalkd server and each
//! connection will be considered separate.
//!
//! The protocol is ASCII text based but the data itself is just a bytestream. This means that the
//! application is responsible for interpreting the data.
//!
//! ## Operation
//! This library can serve as a client for both the application and the worker. The application would
//! [Beanstalkd::put] jobs on the queue and the workers can [Beanstalkd::reserve]
//! them. Once they are done with the job, they have to [Beanstalkd::delete] job.
//! This is required for every job, or else Beanstalkd will not remove it fromits internal datastructres.
//!
//! If a worker cannot finish the job in it's TTR (Time To Run), then it can [Beanstalkd::release]
//! the job. The application can use the [Beanstalkd::using] method to put jobs in a specific tube,
//! and workers can use [Beanstalkd::watch]
//!
//! ## Interaction with Tokio
//!
//! The futures in this crate expect to be running under a `tokio::Runtime`. In the common case,
//! you cannot resolve them solely using `.wait()`, but should instead use `tokio::run` or
//! explicitly create a `tokio::Runtime` and then use `Runtime::block_on`.
//!
//! An simple example client could look something like this:
//!
//! ```no_run
//! # use tokio_beanstalkd::*;
//! #[tokio::main]
//! async fn main() {
//!     let mut bean = Beanstalkd::connect(
//!         &"127.0.0.1:11300"
//!             .parse()
//!             .expect("Unable to connect to Beanstalkd"),
//!     )
//!     .await
//!     .unwrap();
//!
//!     bean.put(0, 1, 100, &b"update:42"[..]).await.unwrap();
//!
//!     // Use a particular tube
//!     bean.using("notifications").await.unwrap();
//!     bean.put(0, 1, 100, &b"notify:100"[..]).await.unwrap();
//! }
//! ```
//!
//! And a worker could look something like this:
//! ```no_run
//! # use tokio_beanstalkd::*;
//! #[tokio::main]
//! async fn main() {
//!     let mut bean = Beanstalkd::connect(
//!         &"127.0.0.1:11300"
//!             .parse()
//!             .expect("Unable to connect to Beanstalkd"),
//!     )
//!     .await
//!     .unwrap();
//!
//!     let response = bean.reserve().await.unwrap();
//!     // ... do something with the response ...
//!     // Delete the job once it is done
//!     bean.delete(response.id).await.unwrap();
//! }
//! ```

#![warn(rust_2018_idioms)]

pub mod errors;
mod proto;

use std::borrow::Cow;
use std::net::SocketAddr;

use futures::SinkExt;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

pub use crate::proto::response::*;
pub use crate::proto::{Id, Tube};
// Request doesn't have to be a public type
use crate::proto::Request;

use crate::errors::{BeanstalkError, Consumer, Put};

/// Connection to the Beanstalkd server.
///
/// All interactions with Beanstalkd happen by calling methods on a `Beanstalkd` instance.
///
/// Even though there is a `quit` command, Beanstalkd consideres a closed connection as the
/// end of communication, so just dropping this struct will close the connection.
#[derive(Debug)]
pub struct Beanstalkd {
    connection: Framed<tokio::net::TcpStream, proto::CommandCodec>,
}

pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

// FIXME: log out unexpected errors using env_logger
impl Beanstalkd {
    /// Connect to a Beanstalkd instance.
    ///
    /// A successful TCP connect is considered the start of communication.
    pub async fn connect(addr: &SocketAddr) -> Result<Self, Error> {
        let c = tokio::net::TcpStream::connect(addr).await?;
        Ok(Beanstalkd::setup(c))
    }

    fn setup(stream: tokio::net::TcpStream) -> Self {
        let bean = Framed::new(stream, proto::CommandCodec::new());
        Beanstalkd { connection: bean }
    }

    async fn response(&mut self) -> Result<Response, proto::error::BeanError> {
        use proto::error::{BeanError, ProtocolError};

        match self.connection.next().await {
            Some(r) => r,
            None => Err(BeanError::Protocol(ProtocolError::StreamClosed)),
        }
    }

    /// The "put" command is for any process that wants to insert a job into the queue.
    ///
    /// It inserts a job into the client's currently used tube (see the `use` command
    /// below).
    ///
    /// - `priority` is an integer < 2**32. Jobs with smaller priority values will be
    ///   scheduled before jobs with larger priorities. The most urgent priority is 0;
    ///   the least urgent priority is 4,294,967,295.
    /// - `delay` is an integer number of seconds to wait before putting the job in
    ///   the ready queue. The job will be in the "delayed" state during this time.
    /// - `ttr` -- time to run -- is an integer number of seconds to allow a worker
    ///   to run this job. This time is counted from the moment a worker reserves
    ///   this job. If the worker does not delete, release, or bury the job within
    ///   `ttr` seconds, the job will time out and the server will release the job.
    ///   The minimum ttr is 1. If the client sends 0, the server will silently
    ///   increase the ttr to 1.
    /// - `data` is the job body -- a sequence of bytes of length \<bytes\> from the
    ///   previous line.
    ///
    /// After sending the command line and body, the client waits for a reply, which
    /// is the integer id of the new job.
    pub async fn put<D>(
        &mut self,
        priority: u32,
        delay: u32,
        ttr: u32,
        data: D,
    ) -> Result<Response, Put>
    where
        D: Into<Cow<'static, [u8]>>,
    {
        let data = data.into();
        self.connection
            .send(proto::Request::Put {
                priority,
                delay,
                ttr,
                data,
            })
            .await?;

        self.response().await.map_err(Into::into)
    }

    /// Reserve a [proto::response::Job] to process.
    ///
    /// FIXME: need to handle different responses returned at different TTR vs reserve-with-timeout times
    ///
    /// A process that wants to consume jobs from the queue uses `reserve`,
    /// `[Beanstalkd::delete]`,
    /// `[Beanstalkd::release]`, and
    /// `[Beanstalkd::bury]`.
    pub async fn reserve(&mut self) -> Result<Response, Consumer> {
        self.connection.send(proto::Request::Reserve).await?;

        self.response().await.map_err(Into::into)
    }

    /// The "use" command is for producers. Subsequent put commands will put jobs into
    /// the tube specified by this command. If no use command has been issued, jobs
    /// will be put into the tube named "default".
    ///
    /// - `tube` is a name at most 200 bytes. It specifies the tube to use. If the
    ///   tube does not exist, it will be created.
    pub async fn using(&mut self, tube: &'static str) -> Result<Response, BeanstalkError> {
        self.connection.send(Request::Use { tube }).await?;

        self.response().await.map_err(Into::into)
    }

    /// The delete command removes a job from the server entirely. It is normally used
    /// by the client when the job has successfully run to completion. A client can
    /// delete jobs that it has reserved, ready jobs, delayed jobs, and jobs that are
    /// buried.
    ///
    ///  - `id` is the job id to delete.
    pub async fn delete(&mut self, id: u32) -> Result<Response, errors::Consumer> {
        self.connection.send(Request::Delete { id }).await?;

        self.response().await.map_err(Into::into)
    }

    /// The release command puts a reserved job back into the ready queue (and marks
    /// its state as "ready") to be run by any client. It is normally used when the job
    /// fails because of a transitory error.
    ///
    ///  - `id` is the job id to release.
    ///
    /// - `pri` is a new priority to assign to the job.
    ///
    /// - `delay` is an integer number of seconds to wait before putting the job in
    ///   the ready queue. The job will be in the "delayed" state during this time.
    pub async fn release(
        &mut self,
        id: u32,
        priority: u32,
        delay: u32,
    ) -> Result<Response, Consumer> {
        self.connection
            .send(Request::Release {
                id,
                priority,
                delay,
            })
            .await?;

        self.response().await.map_err(Into::into)
    }

    /// The "touch" command allows a worker to request more time to work on a job.
    /// This is useful for jobs that potentially take a long time, but you still want
    /// the benefits of a TTR pulling a job away from an unresponsive worker.  A worker
    /// may periodically tell the server that it's still alive and processing a job
    /// (e.g. it may do this on DEADLINE_SOON). The command postpones the auto
    /// release of a reserved job until TTR seconds from when the command is issued.
    ///
    /// - `id` is the ID of a job reserved by the current connection.
    pub async fn touch(&mut self, id: u32) -> Result<Response, Consumer> {
        self.connection.send(Request::Touch { id }).await?;

        self.response().await.map_err(Into::into)
    }

    /// The bury command puts a job into the "buried" state. Buried jobs are put into a
    /// FIFO linked list and will not be touched by the server again until a client
    /// kicks them with the "kick" command.
    ///
    ///  - `id` is the job id to release.
    ///
    /// - `prioritiy` is a new priority to assign to the job.
    pub async fn bury(&mut self, id: u32, priority: u32) -> Result<Response, Consumer> {
        self.connection.send(Request::Bury { id, priority }).await?;

        self.response().await.map_err(Into::into)
    }

    /// The "watch" command adds the named tube to the watch list for the current
    /// connection. A reserve command will take a job from any of the tubes in the
    /// watch list. For each new connection, the watch list initially consists of one
    /// tube, named "default".
    ///
    ///  - \<tube\> is a name at most 200 bytes. It specifies a tube to add to the watch
    ///     list. If the tube doesn't exist, it will be created.
    ///
    /// The value returned is the count of the tubes being watched by the current connection.
    pub async fn watch(&mut self, tube: &'static str) -> Result<Response, BeanstalkError> {
        self.connection.send(Request::Watch { tube }).await?;

        self.response().await.map_err(Into::into)
    }

    /// The "ignore" command is for consumers. It removes the named tube from the
    /// watch list for the current connection.
    ///
    ///  - \<tube\> is a name at most 200 bytes. It specifies a tube to add to the watch
    ///     list. If the tube doesn't exist, it will be created.
    ///
    /// A successful response is:
    ///
    /// - The count of the number of tubes currently watching
    pub async fn ignore(&mut self, tube: &'static str) -> Result<Response, Consumer> {
        self.connection.send(Request::Ignore { tube }).await?;

        self.response().await.map_err(Into::into)
    }

    /// The peek command lets the client inspect a job in the system. There are four
    /// types of jobs as enumerated in [PeekType]. All but the
    /// first operate only on the currently used tube.
    ///
    /// * It takes a [PeekType] representing the type of peek
    /// operation to perform
    ///
    /// * And returns a [proto::response::Job] on success.
    pub async fn peek(&mut self, peek_type: PeekType) -> Result<Response, Consumer> {
        let request = match peek_type {
            PeekType::Ready => Request::PeekReady,
            PeekType::Delayed => Request::PeekDelay,
            PeekType::Buried => Request::PeekBuried,
            PeekType::Normal(id) => Request::Peek { id },
        };

        self.connection.send(request).await?;

        self.response().await.map_err(Into::into)
    }

    /// The kick command applies only to the currently used tube. It moves jobs into
    /// the ready queue. If there are any buried jobs, it will only kick buried jobs.
    /// Otherwise it will kick delayed jobs.
    ///
    /// * It takes a `bound` which is the number of jobs it will kick
    /// * The response is a u32 representing the number of jobs kicked by the server
    pub async fn kick(&mut self, bound: u32) -> Result<Response, Consumer> {
        self.connection.send(Request::Kick { bound }).await?;

        self.response().await.map_err(Into::into)
    }

    /// The kick-job command is a variant of kick that operates with a single job
    /// identified by its job id. If the given job id exists and is in a buried or
    /// delayed state, it will be moved to the ready queue of the the same tube where it
    /// currently belongs.
    ///
    /// * It takes an `id` of the job to be kicked
    /// * And returns `()` on success
    pub async fn kick_job(&mut self, id: Id) -> Result<Response, Consumer> {
        self.connection.send(Request::KickJob { id }).await?;

        self.response().await.map_err(Into::into)
    }
}

/// The type of [Beanstalkd::peek] request you want to make
pub enum PeekType {
    /// The next ready job
    Ready,

    /// The next delayed job
    Delayed,

    /// The next bufied job
    Buried,

    /// The job with the given Id
    Normal(Id),
}
