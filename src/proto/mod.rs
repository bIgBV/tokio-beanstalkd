use bytes::BytesMut;
use failure;
use failure::ResultExt;
use tokio::codec::{Decoder, Encoder};

use std::io;
use std::str;
use std::str::FromStr;

pub mod error;
mod request;
pub(crate) mod response;

pub(crate) use self::request::Request;
pub use self::response::*;

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

    fn parse_response(&self, list: Vec<&str>) -> Result<AnyResponse, error::DecoderError> {
        eprintln!("Parsing: {:?}", list);
        if list.len() == 1 {
            return match list[0] {
                "OUT_OF_MEMORY" => Err(error::ErrorKind::Protocol(
                    error::ProtocolError::OutOfMemory,
                ))?,
                "INTERNAL_ERROR" => Err(error::ErrorKind::Protocol(
                    error::ProtocolError::InternalError,
                ))?,
                "BAD_FORMAT" => Err(error::ErrorKind::Protocol(error::ProtocolError::BadFormat))?,
                "UNKNOWN_COMMAND" => Err(error::ErrorKind::Protocol(
                    error::ProtocolError::UnknownCommand,
                ))?,
                "EXPECTED_CRLF" => Err(error::ErrorKind::Protocol(
                    error::ProtocolError::ExpectedCRLF,
                ))?,
                "JOB_TOO_BIG" => Err(error::ErrorKind::Protocol(error::ProtocolError::JobTooBig))?,
                "DRAINING" => Err(error::ErrorKind::Protocol(error::ProtocolError::Draining))?,
                "NOT_FOUND" => Err(error::ErrorKind::Protocol(error::ProtocolError::NotFound))?,
                "NOT_IGNORED" => Err(error::ErrorKind::Protocol(error::ProtocolError::NotIgnored))?,
                "BURIED" => Ok(AnyResponse::Buried),
                "TOUCHED" => Ok(AnyResponse::Touched),
                "RELEASED" => Ok(AnyResponse::Released),
                "DELETED" => Ok(AnyResponse::Deleted),
                _ => Err(error::ErrorKind::Parsing(
                    error::ParsingError::UnknownResponse,
                ))?,
            };
        }

        if list.len() == 2 {
            eprintln!("Parsing: {:?}", list[1]);
            return match list[0] {
                "INSERTED" => {
                    let id: u32 = u32::from_str(list[1])
                        .context(error::ErrorKind::Parsing(error::ParsingError::ParseId))?;
                    Ok(AnyResponse::Inserted(id))
                }
                "WATCHING" => {
                    let count = u32::from_str(list[1])
                        .context(error::ErrorKind::Parsing(error::ParsingError::ParseId))?;
                    Ok(AnyResponse::Watching(count))
                }
                "USING" => Ok(AnyResponse::Using(String::from(list[1]))),
                _ => Err(error::ErrorKind::Parsing(
                    error::ParsingError::UnknownResponse,
                ))?,
            };
        }

        if list.len() == 3 {
            return match list[0] {
                "RESERVED" => Ok(AnyResponse::Pre(parse_pre_job(list[1..].to_vec())?)),
                _ => Err(error::ErrorKind::Parsing(
                    error::ParsingError::UnknownResponse,
                ))?,
            };
        }

        Err(error::ErrorKind::Parsing(
            error::ParsingError::UnknownResponse,
        ))?
    }

    fn parse_job(
        &mut self,
        src: &mut BytesMut,
        pre: PreJob,
    ) -> Result<Option<Job>, error::DecoderError> {
        if let Some(carriage_offset) = src.iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                let line =
                    utf8(src).context(error::ErrorKind::Parsing(error::ParsingError::ParseString))?;
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

fn parse_pre_job(list: Vec<&str>) -> Result<PreJob, error::DecoderError> {
    let id =
        u32::from_str(list[0]).context(error::ErrorKind::Parsing(error::ParsingError::ParseId))?;
    let bytes =
        usize::from_str(list[1]).context(error::ErrorKind::Parsing(error::ParsingError::ParseId))?;
    Ok(PreJob { id, bytes })
}

impl Decoder for CommandCodec {
    type Item = AnyResponse;
    type Error = error::DecoderError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        eprintln!("Decoding: {:?}", src);
        if let Some(carriage_offset) = src[self.outstart..].iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                // Afterwards src contains elements [at, len), and the returned BytesMut
                // contains elements [0, at), so + 1 for \r and then +1 for \n
                let offset = self.outstart + carriage_offset + 1 + 1;
                let line = src.split_to(offset);
                let line = utf8(&line)
                    .context(error::ErrorKind::Parsing(error::ParsingError::ParseString))?;
                let line = line.trim().split(" ").collect();

                let response = self.parse_response(line)?;

                eprintln!("Got response: {:?}", response);
                // Since the actual job data is on a second line, we need additional parsing
                // extract it from the buffer.
                match response {
                    AnyResponse::Pre(pre) => {
                        if let Some(job) = self.parse_job(src, pre)? {
                            self.outstart = 0;
                            src.clear();
                            return Ok(Some(AnyResponse::Reserved(job)));
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

        // TODO: Just Ok(None) is enough
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
            }
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
