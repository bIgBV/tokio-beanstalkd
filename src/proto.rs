use bytes::{BufMut, BytesMut};
use failure;
use tokio::codec::{Decoder, Encoder};
use tokio::prelude::*;

use std::fmt::{self, Debug, Display};
use std::io;
use std::str::FromStr;

pub(crate) struct Packetizer {
    /// Prefix of outbox that has been sent
    outstart: usize,
}

impl Packetizer {
    pub(crate) fn new() -> Packetizer {
        Packetizer { outstart: 0 }
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

impl Encoder for Packetizer {
    type Item = Request;
    type Error = failure::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match *self {
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
        return match **list[0] {
            "OUT_OF_MEMORY" => Response::OutOfMemory,
            "INTERNAL_ERROR" => Response::InternalError,
            "BAD_FORMAT" => Response::BadFormat,
            "UNKNOWN_COMMAND" => Response::UnknownCommand,
            "EXPECTED_CRLF" => Response::ExpectedCLRF,
            "JOB_TOO_BIG" => Response::JobTooBig,
            "DRAINING" => Response::Draining,
            _ => Err(unimplemented!()),
        };
    }

    if list.len() == 2 {
        let id = FromStr::from_str::<u64>(**list[1]);
        return match **list[0] {
            "INSERTED" => Response::Inserted(id),
            "BURIED" => Response::Buried(id),
            _ => Err(unimplemented!()),
        };
    }

    // TODO Consumer commands
}

impl Decoder for Packetizer {
    type Item = Response;
    type Error = failure::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(carriage_offset) = src[self.outstart..].iter().position(|b| *b == b'\r') {
            let iter = src.iter().skip(carriage_offset).peekable();
            if let Some(val) = iter.peek() {
                if **val == b'\n' {
                    let delimitter_offset = carriage_offset + 1;
                    let line = utf8(src.split_to(self.outstart + delimitter_offset))?;
                    let line = line.trim().split(" ").collect();
                    return Ok(Some(parse_response(line)?));
                } else {
                    self.outstart = src.len();
                    return Ok(None);
                }
            }
        } else {
            self.outstart = src.len();
            Ok(None)
        }
    }
}
