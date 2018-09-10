/// This crate provides a client for working with [Beanstalkd](https://beanstalkd.github.io/), a simple
/// fast work queue.
///
/// # About Beanstalkd
///
extern crate bytes;
extern crate futures;
#[macro_use]
extern crate failure;
extern crate tokio;

mod proto;

use tokio::codec::Framed;
use tokio::prelude::*;

use std::borrow::Cow;
use std::net::SocketAddr;

pub use proto::Request;
pub use proto::Response;

macro_rules! handle_response {
    ($input:ident) => {
        $input.into_future().then(|val| match val {
            Ok((Some(val), conn)) => Ok((Beanstalkd { connection: conn }, Ok(val))),
            // None is only returned when the stream is closed
            Ok((None, _)) => bail!("Stream closed"),
            Err((e, conn)) => Ok((Beanstalkd { connection: conn }, Err(e))),
        })
    };
}

pub struct Beanstalkd {
    connection: Framed<tokio::net::TcpStream, proto::CommandCodec>,
}

impl Beanstalkd {
    pub fn connect(addr: &SocketAddr) -> impl Future<Item = Self, Error = failure::Error> {
        tokio::net::TcpStream::connect(addr)
            .map_err(failure::Error::from)
            .map(|stream| Beanstalkd::setup(stream))
    }

    fn setup(stream: tokio::net::TcpStream) -> Self {
        let bean = Framed::new(stream, proto::CommandCodec::new());
        Beanstalkd { connection: bean }
    }

    pub fn put<D>(
        self,
        priority: u32,
        delay: u32,
        ttr: u32,
        data: D,
    ) -> impl Future<Item = (Self, Result<proto::Response, failure::Error>), Error = failure::Error>
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
            .and_then(|conn| handle_response!(conn))
    }

    pub fn reserve(
        self,
    ) -> impl Future<Item = (Self, Result<proto::Response, failure::Error>), Error = failure::Error>
    {
        self.connection
            .send(proto::Request::Reserve)
            .and_then(|conn| handle_response!(conn))
    }

    pub fn using(
        self,
        tube: &'static str,
    ) -> impl Future<Item = (Self, Result<proto::Response, failure::Error>), Error = failure::Error>
    {
        self.connection
            .send(Request::Use { tube })
            .and_then(|conn| handle_response!(conn))
    }

    pub fn delete(
        self,
        id: u32,
    ) -> impl Future<Item = (Self, Result<proto::Response, failure::Error>), Error = failure::Error>
    {
        self.connection
            .send(Request::Delete { id })
            .and_then(|conn| handle_response!(conn))
    }

    pub fn release(
        self,
        id: u32,
        priority: u32,
        delay: u32,
    ) -> impl Future<Item = (Self, Result<Response, failure::Error>), Error = failure::Error> {
        self.connection
            .send(Request::Release {
                id,
                priority,
                delay,
            })
            .and_then(|conn| handle_response!(conn))
    }
}

#[cfg(test)]
mod tests {
    // TODO: spawn a separate process for beanstalkd so the tests don't depend on anything else.
    use super::*;

    #[test]
    fn it_works() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let bean = rt.block_on(
            Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
                // Let put a job in
                bean.put(0, 1, 1, &b"data"[..])
                    .inspect(|(_, response)| assert!(response.is_ok()))
                    .and_then(|(bean, _)| {
                        // how about another one?
                        bean.put(0, 1, 1, &b"more data"[..])
                    })
                    .inspect(|(_, response)| assert!(response.is_ok()))
                    .and_then(|(bean, _)| {
                        // Let's watch a particular tube
                        bean.using("test")
                    })
                    .inspect(|(_, response)| match response {
                        Ok(v) => assert_eq!(*v, Response::Using("test".to_string())),
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
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let bean = rt.block_on(
            Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
                bean.put(0, 1, 1, &b"data"[..])
                    .inspect(|(_, response)| assert!(response.is_ok()))
                    .and_then(|(bean, _)| bean.reserve())
                    .inspect(|(_, response)| match response {
                        Ok(proto::Response::Reserved(j)) => {
                            assert_eq!(j.data, b"data");
                        }
                        _ => panic!("Wrong response received"),
                    })
                    .and_then(|(bean, _)| {
                        // how about another one?
                        bean.put(0, 1, 1, &b"more data"[..])
                    })
                    .inspect(|(_, response)| assert!(response.is_ok()))
                    .and_then(|(bean, response)| match response {
                        Ok(Response::Inserted(id)) => bean.delete(id),
                        Ok(_) => panic!("Wrong response returned"),
                        Err(e) => panic!("Got error: {}", e),
                    })
                    .inspect(|(_, response)| match response {
                        Ok(v) => assert_eq!(*v, Response::Deleted),
                        Err(e) => {
                            // assert_eq!(*e, proto::error::Consumer::NotFound);
                            panic!("Got error: {}", e)
                        }
                    })
            }),
        );
        assert!(!bean.is_err());
        drop(bean);
        rt.shutdown_on_idle();
    }
}
