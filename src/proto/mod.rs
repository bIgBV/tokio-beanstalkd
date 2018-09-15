use bytes::BytesMut;
use failure;
use tokio::codec::{Decoder, Encoder};

use std::io;
use std::str;
use std::str::FromStr;

pub mod error;
mod request;
mod response;

use self::error::BeanstalkError;
pub(crate) use self::request::Request;
pub use self::response::Response;

use self::response::{Job, PreJob};

pub type Tube = String;

pub type Id = u32;

#[derive(Debug, Clone)]
pub(crate) struct CommandCodec {
    /// Prefix of outbox that has been sent
    outstart: usize,
}

impl CommandCodec {
    pub(crate) fn new() -> CommandCodec {
        CommandCodec { outstart: 0 }
    }

    fn parse_response(&self, list: Vec<&str>) -> Result<Response, failure::Error> {
        eprintln!("Parsing: {:?}", list);
        if list.len() == 1 {
            return match list[0] {
                "OUT_OF_MEMORY" => Err(failure::Error::from(BeanstalkError::OutOfMemory)),
                "INTERNAL_ERROR" => Err(failure::Error::from(BeanstalkError::InternalError)),
                "BAD_FORMAT" => Err(failure::Error::from(BeanstalkError::BadFormat)),
                "UNKNOWN_COMMAND" => Err(failure::Error::from(BeanstalkError::UnknownCommand)),
                "EXPECTED_CRLF" => Err(failure::Error::from(error::Put::ExpectedCLRF)),
                "JOB_TOO_BIG" => Err(failure::Error::from(error::Put::JobTooBig)),
                "DRAINING" => Err(failure::Error::from(error::Put::Draining)),
                "NOT_FOUND" => Err(failure::Error::from(error::Consumer::NotFound)),
                "NOT_IGNORED" => Err(failure::Error::from(error::Consumer::NotIgnored)),
                "BURIED" => Ok(Response::Buried),
                "TOUCHED" => Ok(Response::Touched),
                "RELEASED" => Ok(Response::Released),
                "DELETED" => Ok(Response::Deleted),
                _ => bail!("Unknown response from server"),
            };
        }

        if list.len() == 2 {
            eprintln!("Parsing: {:?}", list[1]);
            return match list[0] {
                "INSERTED" => {
                    let id = FromStr::from_str(list[1])?;
                    Ok(Response::Inserted(id))
                },
                "WATCHING" => {
                    let count = FromStr::from_str(list[1])?;
                    Ok(Response::Watching(count))
                }
                "USING" => Ok(Response::Using(String::from(list[1]))),
                _ => bail!("Unknown resonse from server"),
            };
        }

        if list.len() == 3 {
            return match list[0] {
                "RESERVED" => Ok(Response::Pre(parse_pre_job(list[1..].to_vec())?)),
                _ => bail!("Unknown response from server."),
            };
        }

        bail!("Unable to parse response")
    }

    fn parse_job(
        &mut self,
        src: &mut BytesMut,
        pre: PreJob,
    ) -> Result<Option<Job>, failure::Error> {
        if let Some(carriage_offset) = src.iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                let line = utf8(src)?;
                let line: Vec<&str> = line.trim().split(" ").collect();
                return Ok(Some(Job {
                    id: pre.id,
                    bytes: pre.bytes,
                    data: line[0].as_bytes().to_vec(),
                }));
            }
        }
        self.outstart += src.len();
        Ok(None)
    }
}

fn parse_pre_job(list: Vec<&str>) -> Result<PreJob, failure::Error> {
    let id = u32::from_str(list[0])?;
    let bytes = usize::from_str(list[1])?;
    Ok(PreJob { id, bytes })
}

impl Decoder for CommandCodec {
    type Item = Response;
    type Error = failure::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        eprintln!("Decoding: {:?}", src);
        if let Some(carriage_offset) = src[self.outstart..].iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                // Afterwards src contains elements [at, len), and the returned BytesMut
                // contains elements [0, at), so + 1 for \r and then +1 for \n
                let offset = self.outstart + carriage_offset + 1 + 1;
                let line = src.split_to(offset);
                let line = utf8(&line)?;
                let line = line.trim().split(" ").collect();

                let response = self.parse_response(line)?;

                eprintln!("Got response: {:?}", response);
                // Since the actual job data is on a second line, we need additional parsing
                // extract it from the buffer.
                match response {
                    Response::Pre(pre) => {
                        if let Some(job) = self.parse_job(src, pre)? {
                            self.outstart = 0;
                            src.clear();
                            return Ok(Some(Response::Reserved(job)));
                        } else {
                            return Ok(None);
                        }
                    }
                    _ => {
                        self.outstart = 0;
                        src.clear();
                        return Ok(Some(response));
                    }
                };
            }
        }
        self.outstart = src.len();
        src.clear();
        return Ok(None);
    }
}

impl Encoder for CommandCodec {
    type Item = Request;
    type Error = failure::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        eprintln!("Making request: {:?}", item);
        match item {
            Request::Watch { tube } => {
                if tube.as_bytes().len() > 200 {
                    bail!("Tube name too long")
                }
                item.serialize(dst)
            },
            _ => item.serialize(dst),
        }
        Ok(())
    }
}

fn utf8(buf: &[u8]) -> Result<&str, io::Error> {
    str::from_utf8(buf)
        // This should never happen since everything is ascii
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unable to decode input as UTF8"))
}
