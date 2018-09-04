extern crate failure;
#[macro_use]
extern crate futures;
extern crate bytes;
extern crate tokio;

mod proto;

use failure::ResultExt;
use tokio::codec::Framed;
use tokio::prelude::*;

use std::net::SocketAddr;

pub struct Beanstalkd<S> {
    connection: Framed<S, proto::CommandCodec>,
}

impl<S> Beanstalkd<S>
where
    S: AsyncRead + AsyncWrite,
{
    pub fn connect(
        addr: &SocketAddr,
    ) -> impl Future<Item = Beanstalkd<tokio::net::TcpStream>, Error = failure::Error> {
        tokio::net::TcpStream::connect(addr)
            .map_err(failure::Error::from)
            .and_then(|stream| Beanstalkd::setup(stream))
    }

    fn setup(stream: S) -> Self {
        let bean = Framed::new(stream, proto::CommandCodec::new());
        Beanstalkd { connection: bean }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let bean = tokio::run(Beanstalkd::connect("127.0.0.1:11300".parse().unwrap()));
    }
}
