use bytes::{BufMut, BytesMut};
use failure;
use tokio::codec::{Decoder, Encoder};
use tokio::prelude::*;

use std::fmt::{self, Debug, Display};
use std::io;
use std::str;
use std::str::FromStr;

pub(crate) struct CommandCodec {
    /// Prefix of outbox that has been sent
    outstart: usize,
}

impl CommandCodec {
    pub(crate) fn new() -> CommandCodec {
        CommandCodec { outstart: 0 }
    }
}

pub(crate) enum Request {
    Put {
        priority: u32,
        delay: u32,
        ttr: u32,
        data: &'static str,
    },
}

pub type Tube = String;

pub type Id = u64;

#[derive(Debug)]
pub(crate) enum Response {
    OK,
    Reserved,
    Inserted(Id),
    Buried(Id),
    Using(Tube),
    Deleted,
    Watching,
    NotIgnored,
    BadFormat,
    ExpectedCLRF,
    JobTooBig,
    Draining,
    OutOfMemory,
    InternalError,
    UnknownCommand,
}

impl Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
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
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unable to decode input as UTF8"))
}

fn parse_response(list: &[&str]) -> Result<Response, failure::Error> {
    if list.len() == 1 {
        return match list[0] {
            "OUT_OF_MEMORY" => Ok(Response::OutOfMemory),
            "INTERNAL_ERROR" => Ok(Response::InternalError),
            "BAD_FORMAT" => Ok(Response::BadFormat),
            "UNKNOWN_COMMAND" => Ok(Response::UnknownCommand),
            "EXPECTED_CRLF" => Ok(Response::ExpectedCLRF),
            "JOB_TOO_BIG" => Ok(Response::JobTooBig),
            "DRAINING" => Ok(Response::Draining),
            _ => Err(unimplemented!()),
        };
    }

    if list.len() == 2 {
        let id = FromStr::from_str(list[1])?;
        return match list[0] {
            "INSERTED" => Ok(Response::Inserted(id)),
            "BURIED" => Ok(Response::Buried(id)),
            _ => Err(unimplemented!()),
        };
    }
    // TODO Consumer commands
    //
    Err(unimplemented!())
}

impl Decoder for CommandCodec {
    type Item = Response;
    type Error = failure::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(carriage_offset) = src[self.outstart..].iter().position(|b| *b == b'\r') {
            let iter = src.iter().skip(carriage_offset).peekable();
            if let Some(val) = iter.peek() {
                if **val == b'\n' {
                    let delimitter_offset = carriage_offset + 1;
                    let line = utf8(&src.split_to(self.outstart + delimitter_offset))?;
                    let line = line.trim().split(" ").collect();
                    return Ok(Some(parse_response(line)?));
                } else {
                    self.outstart = src.len();
                    return Ok(None);
                }
            }
        } else {
            self.outstart = src.len();
            return Ok(None);
        }

        Ok(None)
    }
}
