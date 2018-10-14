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
//! `put` jobs on the queue and the workers can `reserve` them. Once they are done with the job, they
//! have to `delete` job. This is required for every job, or else beanstlkd will not remove it from
//! its internal datastructres.
//!
//! If a worker cannot finish the job in it's TTR (Time To Run), then it can `release` the job. The
//! application can use the `using` method to put jobs in a specific tube, and workers can use `watch`
//! to only reserve jobs from the specified tubes.
//!
//! ## Interaction with Tokio
//!
//! The futures in this crate expect to be running under a `tokio::Runtime`. In the common case,
//! you cannot resolve them solely using `.wait()`, but should instead use `tokio::run` or
//! explicitly create a `tokio::Runtime` and then use `Runtime::block_on`.
//!
//! A contrived example
//!
//! ```no_run
//! extern crate tokio;
//! extern crate futures;
//! extern crate tokio_beanstalkd;
//!
//! use tokio::prelude::*;
//! use tokio_beanstalkd::*;
//!
//! # fn consumer_commands() {
//!      let mut rt = tokio::runtime::Runtime::new().unwrap();
//!      let bean = rt.block_on(
//!          Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
//!              bean.put(0, 1, 100, &b"data"[..])
//!                  .inspect(|(_, response)| {
//!                      response.as_ref().unwrap();
//!                  }).and_then(|(bean, _)| bean.reserve())
//!                  .inspect(|(_, response)| assert_eq!(response.as_ref().unwrap().data, b"data"))
//!                  .and_then(|(bean, response)| bean.touch(response.unwrap().id))
//!                  .inspect(|(_, response)| {
//!                      response.as_ref().unwrap();
//!                  }).and_then(|(bean, _)| {
//!                      // how about another one?
//!                      bean.put(0, 1, 100, &b"more data"[..])
//!                  }).and_then(|(bean, _)| bean.reserve())
//!                  .and_then(|(bean, response)| bean.release(response.unwrap().id, 10, 10))
//!                  .inspect(|(_, response)| {
//!                      response.as_ref().unwrap();
//!                  }).and_then(|(bean, _)| bean.reserve())
//!                  .and_then(|(bean, response)| bean.bury(response.unwrap().id, 10))
//!                  .inspect(|(_, response)| {
//!                      response.as_ref().unwrap();
//!                  }).and_then(|(bean, _)| {
//!                      // how about another one?
//!                      bean.put(0, 1, 100, &b"more data"[..])
//!                  }).inspect(|(_, response)| {
//!                      response.as_ref().unwrap();
//!                  }).and_then(|(bean, response)| bean.delete(response.unwrap()))
//!                  .inspect(|(_, response)| {
//!                      // assert_eq!(*e, error::Consumer::NotFound);
//!                      response.as_ref().unwrap();
//!                  }).and_then(|(bean, _)| bean.watch("test"))
//!                  .inspect(|(_, response)| assert_eq!(*response.as_ref().unwrap(), 2))
//!                  .and_then(|(bean, _)| bean.ignore("test"))
//!                  .inspect(|(_, response)| {
//!                      assert_eq!(response.as_ref().unwrap(), &IgnoreResponse::Watching(1))
//!                  })
//!          }),
//!      );
//!      assert!(!bean.is_err());
//!      drop(bean);
//!      rt.shutdown_on_idle();
//! # }
//! ```

extern crate bytes;
extern crate futures;
#[macro_use]
extern crate failure;
extern crate tokio;

pub mod errors;
mod proto;

use tokio::codec::Framed;
use tokio::prelude::*;

use std::borrow::Cow;
use std::net::SocketAddr;

use proto::error as proto_error;
pub use proto::response::*;
pub use proto::{Id, Tube};
use proto_error::{ErrorKind, ParsingError, ProtocolError};
// Request doesn't have to be a public type
use proto::Request;

use errors::{BeanstalkError, Consumer, Put};

/// A macro to map internal types to external types. The response from a Stream is a 
/// `(Result<Option<Decoder::Item>, Decoder::Error, connection)`. This value needs to be 
/// maapped to the types that we expect. As we can see there are three possible outcomes,
/// and each outcome where there is either a response or an error from the server, we get
/// either an AnyResponse or a proto_error::ProtocolError which need to mapped to simpler
/// public facing types. That is what the two mappings are. The first one is when you get
/// Some(response) and the second one is purely for when the Decoder returns an Error
macro_rules! handle_response {
    ($input:ident, $mapping:tt, $error_mapping:tt) => {
        $input.into_future().then(|val| match val {
            Ok((Some(val), conn)) => Ok((Beanstalkd { connection: conn }, match val $mapping)),
            // None is only returned when the stream is closed
            Ok((None, _)) => bail!("Stream closed"),
            Err((e, conn)) => Ok((Beanstalkd { connection: conn }, match e.kind() $error_mapping)),
        })
    };
}

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

impl Beanstalkd {
    /// Connect to a Beanstalkd instance.
    ///
    /// A successful TCP connect is considered the start of communication.
    pub fn connect(addr: &SocketAddr) -> impl Future<Item = Self, Error = failure::Error> {
        tokio::net::TcpStream::connect(addr)
            .map_err(failure::Error::from)
            .map(|stream| Beanstalkd::setup(stream))
    }

    fn setup(stream: tokio::net::TcpStream) -> Self {
        let bean = Framed::new(stream, proto::CommandCodec::new());
        Beanstalkd { connection: bean }
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

    /// - `data` is the job body -- a sequence of bytes of length <bytes> from the
    ///   previous line.
    ///
    /// After sending the command line and body, the client waits for a reply, which
    /// is the integer id of the new job.
    pub fn put<D>(
        self,
        priority: u32,
        delay: u32,
        ttr: u32,
        data: D,
    ) -> impl Future<Item = (Self, Result<Id, Put>), Error = failure::Error>
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
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Inserted(id) => Ok(id),
                    AnyResponse::Buried => Err(Put::Buried),
                    _r => Err(Put::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                }, {
                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(Put::Beanstalk{error: BeanstalkError::BadFormat}),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(Put::Beanstalk{error: BeanstalkError::OutOfMemory}),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(Put::Beanstalk{error: BeanstalkError::InternalError}),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(Put::Beanstalk{error: BeanstalkError::UnknownCommand}),

                    ErrorKind::Protocol(ProtocolError::ExpectedCRLF) => Err(Put::ExpectedCRLF),
                    ErrorKind::Protocol(ProtocolError::JobTooBig) => Err(Put::JobTooBig),
                    ErrorKind::Protocol(ProtocolError::Draining) => Err(Put::Draining),
                    _r => Err(Put::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                })
            })
    }

    /// Reserve a [job](struct.Job.html) to process.
    ///
    /// A process that wants to consume jobs from the queue uses `reserve`,
    /// [delete](struct.Beanstalkd.html#method.delete),
    /// [release](struct.Beanstalkd.html#method.release), and
    /// [bury](struct.Beanstalkd.html#method.bury).
    pub fn reserve(
        self,
    ) -> impl Future<Item = (Self, Result<Job, Consumer>), Error = failure::Error> {
        self.connection
            .send(proto::Request::Reserve)
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Reserved(job) => Ok(job),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                }, {
                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(Consumer::Beanstalk{error: BeanstalkError::BadFormat}),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(Consumer::Beanstalk{error: BeanstalkError::OutOfMemory}),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(Consumer::Beanstalk{error: BeanstalkError::InternalError}),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(Consumer::Beanstalk{error: BeanstalkError::UnknownCommand}),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                 })
            })
    }

    /// The "use" command is for producers. Subsequent put commands will put jobs into
    /// the tube specified by this command. If no use command has been issued, jobs
    /// will be put into the tube named "default".
    ///
    /// - `tube` is a name at most 200 bytes. It specifies the tube to use. If the
    ///   tube does not exist, it will be created.
    pub fn using(
        self,
        tube: &'static str,
    ) -> impl Future<Item = (Self, Result<Tube, BeanstalkError>), Error = failure::Error> {
        self.connection
            .send(Request::Use { tube })
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Using(tube) => Ok(tube),
                    _r => Err(BeanstalkError::UnexpectedResponse)
                }, {
                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(BeanstalkError::BadFormat),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(BeanstalkError::OutOfMemory),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(BeanstalkError::InternalError),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(BeanstalkError::UnknownCommand),
                    _r => Err(BeanstalkError::UnexpectedResponse)
                 })
            })
    }

    /// The delete command removes a job from the server entirely. It is normally used
    /// by the client when the job has successfully run to completion. A client can
    /// delete jobs that it has reserved, ready jobs, delayed jobs, and jobs that are
    /// buried.
    ///
    ///  - `id` is the job id to delete.
    pub fn delete(
        self,
        id: u32,
    ) -> impl Future<Item = (Self, Result<(), errors::Consumer>), Error = failure::Error> {
        self.connection
            .send(Request::Delete { id })
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Deleted => Ok(()),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                }, {
                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(Consumer::Beanstalk{error: BeanstalkError::BadFormat}),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(Consumer::Beanstalk{error: BeanstalkError::OutOfMemory}),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(Consumer::Beanstalk{error: BeanstalkError::InternalError}),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(Consumer::Beanstalk{error: BeanstalkError::UnknownCommand}),

                    ErrorKind::Protocol(ProtocolError::NotFound) => Err(Consumer::NotFound),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                })
            })
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
    pub fn release(
        self,
        id: u32,
        priority: u32,
        delay: u32,
    ) -> impl Future<Item = (Self, Result<(), Consumer>), Error = failure::Error> {
        self.connection
            .send(Request::Release {
                id,
                priority,
                delay,
            })
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Released => Ok(()),
                    AnyResponse::Buried => Err(Consumer::Buried),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                }, {
                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(Consumer::Beanstalk{error: BeanstalkError::BadFormat}),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(Consumer::Beanstalk{error: BeanstalkError::OutOfMemory}),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(Consumer::Beanstalk{error: BeanstalkError::InternalError}),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(Consumer::Beanstalk{error: BeanstalkError::UnknownCommand}),

                    ErrorKind::Protocol(ProtocolError::NotFound) => Err(Consumer::NotFound),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                })
            })
    }

    /// The "touch" command allows a worker to request more time to work on a job.
    /// This is useful for jobs that potentially take a long time, but you still want
    /// the benefits of a TTR pulling a job away from an unresponsive worker.  A worker
    /// may periodically tell the server that it's still alive and processing a job
    /// (e.g. it may do this on DEADLINE_SOON). The command postpones the auto
    /// release of a reserved job until TTR seconds from when the command is issued.
    ///
    /// - `id` is the ID of a job reserved by the current connection.
    pub fn touch(
        self,
        id: u32,
    ) -> impl Future<Item = (Self, Result<(), Consumer>), Error = failure::Error> {
        self.connection
            .send(Request::Touch { id })
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Touched => Ok(()),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                }, {

                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(Consumer::Beanstalk{error: BeanstalkError::BadFormat}),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(Consumer::Beanstalk{error: BeanstalkError::OutOfMemory}),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(Consumer::Beanstalk{error: BeanstalkError::InternalError}),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(Consumer::Beanstalk{error: BeanstalkError::UnknownCommand}),
                    ErrorKind::Protocol(ProtocolError::NotFound) => Err(Consumer::NotFound),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                })
            })
    }

    /// The bury command puts a job into the "buried" state. Buried jobs are put into a
    /// FIFO linked list and will not be touched by the server again until a client
    /// kicks them with the "kick" command.
    ///
    ///  - `id` is the job id to release.
    ///
    /// - `prioritiy` is a new priority to assign to the job.
    pub fn bury(
        self,
        id: u32,
        priority: u32,
    ) -> impl Future<Item = (Self, Result<(), Consumer>), Error = failure::Error> {
        self.connection
            .send(Request::Bury { id, priority })
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Buried => Ok(()),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                }, {
                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(Consumer::Beanstalk{error: BeanstalkError::BadFormat}),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(Consumer::Beanstalk{error: BeanstalkError::OutOfMemory}),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(Consumer::Beanstalk{error: BeanstalkError::InternalError}),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(Consumer::Beanstalk{error: BeanstalkError::UnknownCommand}),
                    ErrorKind::Protocol(ProtocolError::NotFound) => Err(Consumer::NotFound),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                })
            })
    }

    /// The "watch" command adds the named tube to the watch list for the current
    /// connection. A reserve command will take a job from any of the tubes in the
    /// watch list. For each new connection, the watch list initially consists of one
    /// tube, named "default".
    ///
    ///  - <tube> is a name at most 200 bytes. It specifies a tube to add to the watch
    ///     list. If the tube doesn't exist, it will be created.
    ///
    /// The value returned is the count of the tubes being watched by the current connection.
    pub fn watch(
        self,
        tube: &'static str,
    ) -> impl Future<Item = (Self, Result<u32, BeanstalkError>), Error = failure::Error> {
        self.connection
            .send(Request::Watch { tube })
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Watching(n) => Ok(n),
                    _r => Err(BeanstalkError::UnexpectedResponse)
                }, {

                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(BeanstalkError::BadFormat),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(BeanstalkError::OutOfMemory),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(BeanstalkError::InternalError),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(BeanstalkError::UnknownCommand),
                    _r => Err(BeanstalkError::UnexpectedResponse)
                })
            })
    }

    /// The "ignore" command is for consumers. It removes the named tube from the
    /// watch list for the current connection.
    ///
    ///  - <tube> is a name at most 200 bytes. It specifies a tube to add to the watch
    ///     list. If the tube doesn't exist, it will be created.
    ///
    /// A successful response is:
    ///
    /// - [IgnoreResponse::Watching(u32)](enum.IgnoreResponse.html#variant.Watching)
    ///   The value returned is the count of the tubes being watched by the current connection.
    pub fn ignore(
        self,
        tube: &'static str,
    ) -> impl Future<Item = (Self, Result<u32, Consumer>), Error = failure::Error> {
        self.connection
            .send(Request::Ignore { tube })
            .and_then(|conn| {
                handle_response!(conn, {
                    AnyResponse::Watching(n) => Ok(n),
                    AnyResponse::NotIgnored => Err(errors::Consumer::NotIgnored),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                }, {
                    ErrorKind::Protocol(ProtocolError::BadFormat) => Err(Consumer::Beanstalk{error: BeanstalkError::BadFormat}),
                    ErrorKind::Protocol(ProtocolError::OutOfMemory) => Err(Consumer::Beanstalk{error: BeanstalkError::OutOfMemory}),
                    ErrorKind::Protocol(ProtocolError::InternalError) => Err(Consumer::Beanstalk{error: BeanstalkError::InternalError}),
                    ErrorKind::Protocol(ProtocolError::UnknownCommand) => Err(Consumer::Beanstalk{error: BeanstalkError::UnknownCommand}),
                    ErrorKind::Protocol(ProtocolError::NotFound) => Err(Consumer::NotFound),
                    _r => Err(Consumer::Beanstalk{error: BeanstalkError::UnexpectedResponse})
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    static mut SPAWNED: bool = false;

    // Simple function to make sure only one instance of beanstalkd is spawned.
    // Really sketchy..
    unsafe fn spawn_beanstalkd() {
        if !SPAWNED {
            Command::new("beanstalkd")
                .spawn()
                .expect("Unable to spawn server");
            SPAWNED = true;
        }
    }

    #[test]
    fn it_works() {
        unsafe {
            spawn_beanstalkd();
        }
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let bean = rt.block_on(
            Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
                // Let put a job in
                bean.put(0, 1, 100, &b"data"[..])
                    .inspect(|(_, response)| assert!(response.is_ok()))
                    .and_then(|(bean, _)| {
                        // how about another one?
                        bean.put(0, 1, 100, &b"more data"[..])
                    })
                    .inspect(|(_, response)| assert!(response.is_ok()))
                    .and_then(|(bean, _)| {
                        // Let's watch a particular tube
                        bean.using("test")
                    })
                    .inspect(|(_, response)| match response {
                        Ok(v) => assert_eq!(v, "test"),
                        Err(e) => panic!("Unexpected error: {}", e),
                    })
            }),
        );
        assert!(!bean.is_err());
        drop(bean);
        rt.shutdown_on_idle();
    }

    #[test]
    fn consumer_commands() {
        unsafe {
            spawn_beanstalkd();
        }
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let bean = rt.block_on(
            Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
                bean.put(0, 1, 100, &b"data"[..])
                    .inspect(|(_, response)| {
                        response.as_ref().unwrap();
                    })
                    .and_then(|(bean, _)| bean.reserve())
                    .inspect(|(_, response)| assert_eq!(response.as_ref().unwrap().data, b"data"))
                    .and_then(|(bean, response)| bean.touch(response.unwrap().id))
                    .inspect(|(_, response)| {
                        response.as_ref().unwrap();
                    })
                    .and_then(|(bean, _)| {
                        // how about another one?
                        bean.put(0, 1, 100, &b"more data"[..])
                    })
                    .and_then(|(bean, _)| bean.reserve())
                    .and_then(|(bean, response)| bean.release(response.unwrap().id, 10, 10))
                    .inspect(|(_, response)| {
                        response.as_ref().unwrap();
                    })
                    .and_then(|(bean, _)| bean.reserve())
                    .and_then(|(bean, response)| bean.bury(response.unwrap().id, 10))
                    .inspect(|(_, response)| {
                        response.as_ref().unwrap();
                    })
                    .and_then(|(bean, _)| {
                        // how about another one?
                        bean.put(0, 1, 100, &b"more data"[..])
                    })
                    .inspect(|(_, response)| {
                        response.as_ref().unwrap();
                    })
                    .and_then(|(bean, response)| bean.delete(response.unwrap()))
                    .inspect(|(_, response)| {
                        // assert_eq!(*e, error::Consumer::NotFound);
                        response.as_ref().unwrap();
                    })
                    .and_then(|(bean, _)| bean.watch("test"))
                    .inspect(|(_, response)| assert_eq!(*response.as_ref().unwrap(), 2))
                    .and_then(|(bean, _)| bean.ignore("test"))
                    .inspect(|(_, response)| {
                        assert_eq!(response.as_ref().unwrap(), &IgnoreResponse::Watching(1))
                    })
            }),
        );
        assert!(!bean.is_err());
        drop(bean);
        rt.shutdown_on_idle();
    }
}
