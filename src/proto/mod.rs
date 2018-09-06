use bytes::{BufMut, BytesMut};
use failure;
use tokio::codec::{Decoder, Encoder};

use std::io;
use std::str;
use std::str::FromStr;

mod request;
mod response;

pub(crate) use self::request::Request;
pub(crate) use self::response::Response;

pub(crate) struct CommandCodec {
    /// Prefix of outbox that has been sent
    outstart: usize,
}

impl CommandCodec {
    pub(crate) fn new() -> CommandCodec {
        CommandCodec { outstart: 0 }
    }
}

impl Encoder for CommandCodec {
    type Item = Request;
    type Error = failure::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            Request::Put {
                priority,
                delay,
                ttr,
                data,
            } => {
                let mut foramt_string = format!(
                    "put {pri} {del} {ttr} {len}\r\n{data}\r\n",
                    pri = priority,
                    del = delay,
                    ttr = ttr,
                    len = data.len(),
                    data = data
                );

                dst.reserve(foramt_string.len());
                dst.put(foramt_string.as_bytes());
                Ok(())
            }
        }
    }
}

fn utf8(buf: &[u8]) -> Result<&str, io::Error> {
    str::from_utf8(buf)
        // This should never happen since everything is ascii
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unable to decode input as UTF8"))
}

fn parse_response(list: Vec<&str>) -> Result<Response, failure::Error> {
    if list.len() == 1 {
        return match list[0] {
            "OUT_OF_MEMORY" => Ok(Response::OutOfMemory),
            "INTERNAL_ERROR" => Ok(Response::InternalError),
            "BAD_FORMAT" => Ok(Response::BadFormat),
            "UNKNOWN_COMMAND" => Ok(Response::UnknownCommand),
            "EXPECTED_CRLF" => Ok(Response::ExpectedCLRF),
            "JOB_TOO_BIG" => Ok(Response::JobTooBig),
            "DRAINING" => Ok(Response::Draining),
            _ => bail!("Unknown response from server"),
        };
    }

    if list.len() == 2 {
        let id = FromStr::from_str(list[1])?;
        return match list[0] {
            "INSERTED" => Ok(Response::Inserted(id)),
            "BURIED" => Ok(Response::Buried(id)),
            _ => bail!("Unknown resonse from server"),
        };
    }
    // TODO Consumer commands
    //
    bail!("Unable to parse response")
}

impl Decoder for CommandCodec {
    type Item = Response;
    type Error = failure::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(carriage_offset) = src[self.outstart..].iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                // Afterwards src contains elements [at, len), and the returned BytesMut
                // contains elements [0, at), so + 1 for \r and then +1 for \n
                let line = src.split_to(self.outstart + carriage_offset + 1 + 1);
                let line = utf8(&line)?;
                let line = line.trim().split(" ").collect();
                self.outstart = 0;

                return Ok(Some(parse_response(line)?));
            }
        }
        self.outstart = src.len();
        return Ok(None);
    }
}
