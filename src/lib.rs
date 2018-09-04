extern crate bytes;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let bean = rt.block_on(Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()));
        drop(bean);
        rt.shutdown_on_idle();
    }
}
