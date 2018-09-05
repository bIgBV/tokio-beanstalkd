extern crate bytes;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate tokio;

mod proto;

use failure::ResultExt;
use tokio::codec::Framed;
use tokio::prelude::*;

use std::net::SocketAddr;

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

    pub fn put(
        self,
        priority: u32,
        delay: u32,
        ttr: u32,
        data: &'static str,
    ) -> impl Future<Item = (Self, Result<proto::Response, failure::Error>), Error = failure::Error>
    {
        self.connection
            .send(proto::Request::Put {
                priority,
                delay,
                ttr,
                data,
            })
            .and_then(move |conn| {
                let fut = conn.into_future();

                fut.then(|val| match val {
                    Ok((Some(val), conn)) => Ok(Ok(val)),
                    Ok((None, _)) | Err(_) => bail!("Unable to read from stream"),
                })
            })
            .map(move |r| (self, r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let bean = rt.block_on(
            Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
                bean.put(1, 0, 10, "data")
                    .inspect(|(bean, response)| eprintln!("created {}", response))
            }),
        );
        assert!(!bean.is_err());
        drop(bean);
        rt.shutdown_on_idle();
    }
}
