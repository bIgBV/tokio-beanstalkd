use bytes::BytesMut;
use failure;
use failure::ResultExt;
use tokio::codec::{Decoder, Encoder};

use std::io;
use std::str;
use std::str::FromStr;

pub(crate) mod error;
mod request;
pub(crate) mod response;

pub(crate) use self::request::Request;
pub use self::response::*;

use self::error::{Decode, ErrorKind, ParsingError, ProtocolError};
use self::response::{Job, PreJob};

/// A Tube is a way of separating different types of jobs in Beanstalkd.
///
///  The clinet can use a particular tube by calling [`using`][using] and Beanstalkd will create a
/// new tube if one does not already exist with that name. Workers can [`watch`][watch] particular
/// tubes and receive jobs only from those tubes.
///
/// [using]: struct.Beanstalkd.html#method.using
/// [watch]: struct.Beanstalkd.html#method.watch
pub type Tube = String;

/// The ID of a job assigned by Beanstalkd
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

    /// Helper method which handles all single word responses
    fn single_word_response(&self, list: &[&str]) -> Result<AnyResponse, Decode> {
        match list[0] {
            "OUT_OF_MEMORY" => Err(ErrorKind::Protocol(ProtocolError::OutOfMemory))?,
            "INTERNAL_ERROR" => Err(ErrorKind::Protocol(ProtocolError::InternalError))?,
            "BAD_FORMAT" => Err(ErrorKind::Protocol(ProtocolError::BadFormat))?,
            "UNKNOWN_COMMAND" => Err(ErrorKind::Protocol(ProtocolError::UnknownCommand))?,
            "EXPECTED_CRLF" => Err(ErrorKind::Protocol(ProtocolError::ExpectedCRLF))?,
            "JOB_TOO_BIG" => Err(ErrorKind::Protocol(ProtocolError::JobTooBig))?,
            "DRAINING" => Err(ErrorKind::Protocol(ProtocolError::Draining))?,
            "NOT_FOUND" => Err(ErrorKind::Protocol(ProtocolError::NotFound))?,
            "NOT_IGNORED" => Err(ErrorKind::Protocol(ProtocolError::NotIgnored))?,
            "BURIED" => Ok(AnyResponse::Buried),
            "TOUCHED" => Ok(AnyResponse::Touched),
            "RELEASED" => Ok(AnyResponse::Released),
            "DELETED" => Ok(AnyResponse::Deleted),
            _ => Err(ErrorKind::Parsing(ParsingError::UnknownResponse))?,
        }
    }

    /// Helper method which handles all two word responses
    fn two_word_response(&self, list: &[&str]) -> Result<AnyResponse, Decode> {
        match list[0] {
            "INSERTED" => {
                let id: u32 =
                    u32::from_str(list[1]).context(ErrorKind::Parsing(ParsingError::ParseId))?;
                Ok(AnyResponse::Inserted(id))
            }
            "WATCHING" => {
                let count =
                    u32::from_str(list[1]).context(ErrorKind::Parsing(ParsingError::ParseId))?;
                Ok(AnyResponse::Watching(count))
            }
            "USING" => Ok(AnyResponse::Using(String::from(list[1]))),
            _ => Err(ErrorKind::Parsing(ParsingError::UnknownResponse))?,
        }
    }

    fn parse_response(&self, list: &[&str]) -> Result<AnyResponse, Decode> {
        eprintln!("Parsing: {:?}", list);
        if list.len() == 1 {
            return self.single_word_response(list);
        }

        if list.len() == 2 {
            eprintln!("Parsing: {:?}", list[1]);
            return self.two_word_response(list);
        }

        if list.len() == 3 {
            return match list[0] {
                "RESERVED" => Ok(AnyResponse::Pre(parse_pre_job(
                    &list[1..],
                    response::PreResponse::Reserved,
                )?)),
                _ => Err(ErrorKind::Parsing(ParsingError::UnknownResponse))?,
            };
        }

        Err(ErrorKind::Parsing(ParsingError::UnknownResponse))?
    }

    fn parse_job(&mut self, src: &mut BytesMut, pre: &PreJob) -> Result<Option<Job>, Decode> {
        if let Some(carriage_offset) = src.iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                let line = utf8(src).context(ErrorKind::Parsing(ParsingError::ParseString))?;
                let line: Vec<&str> = line.trim().split(' ').collect();
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

    fn handle_job_response(
        &mut self,
        response: AnyResponse,
        src: &mut BytesMut,
    ) -> Result<Option<AnyResponse>, Decode> {
        if let AnyResponse::Pre(pre) = response {
            if let Some(job) = self.parse_job(src, &pre)? {
                self.outstart = 0;
                src.clear();
                Ok(Some(pre.to_anyresponse(job)))
            } else {
                Ok(None)
            }
        } else {
            self.outstart = 0;
            src.clear();
            Ok(Some(response))
        }
    }
}

fn parse_pre_job(list: &[&str], response_type: response::PreResponse) -> Result<PreJob, Decode> {
    let id = u32::from_str(list[0]).context(ErrorKind::Parsing(ParsingError::ParseId))?;
    let bytes = usize::from_str(list[1]).context(ErrorKind::Parsing(ParsingError::ParseId))?;
    Ok(PreJob {
        id,
        bytes,
        response_type,
    })
}

impl Decoder for CommandCodec {
    type Item = AnyResponse;
    type Error = Decode;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        eprintln!("Decoding: {:?}", src);
        if let Some(carriage_offset) = src[self.outstart..].iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                // Afterwards src contains elements [at, len), and the returned BytesMut
                // contains elements [0, at), so + 1 for \r and then +1 for \n
                let offset = self.outstart + carriage_offset + 1 + 1;
                let line = src.split_to(offset);
                let line = utf8(&line).context(ErrorKind::Parsing(ParsingError::ParseString))?;
                let line: Vec<&str> = line.trim().split(' ').collect();

                let response = self.parse_response(&line[..])?;

                eprintln!("Got response: {:?}", response);
                // Since the actual job data is on a second line, we need additional parsing
                // extract it from the buffer.
                return self.handle_job_response(response, src);
            }
        }
        self.outstart = src.len();
        src.clear();

        Ok(None)
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
