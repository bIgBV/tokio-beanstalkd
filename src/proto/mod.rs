use tokio_util::bytes::BytesMut;
use tokio_util::codec::Decoder;
use tokio_util::codec::Encoder;
use tracing::debug;
use tracing::instrument;
use tracing::trace;

use std::io;
use std::str;
use std::str::FromStr;

pub(crate) mod error;
mod request;
pub(crate) mod response;

pub(crate) use self::request::Request;
pub use self::response::*;

use self::error::{BeanError, EncodeError, ParsingError, ProtocolError};
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
    #[instrument]
    fn single_word_response(&self, list: &[&str]) -> Result<Response, BeanError> {
        let result = match list[0] {
            "OUT_OF_MEMORY" => Err(BeanError::Protocol(ProtocolError::OutOfMemory))?,
            "INTERNAL_ERROR" => Err(BeanError::Protocol(ProtocolError::InternalError))?,
            "BAD_FORMAT" => Err(BeanError::Protocol(ProtocolError::BadFormat))?,
            "UNKNOWN_COMMAND" => Err(BeanError::Protocol(ProtocolError::UnknownCommand))?,
            "EXPECTED_CRLF" => Err(BeanError::Protocol(ProtocolError::ExpectedCRLF))?,
            "JOB_TOO_BIG" => Err(BeanError::Protocol(ProtocolError::JobTooBig))?,
            "DRAINING" => Err(BeanError::Protocol(ProtocolError::Draining))?,
            "NOT_IGNORED" => Err(BeanError::Protocol(ProtocolError::NotIgnored))?,
            "NOT_FOUND" => Ok(Response::NotFound),
            "BURIED" => Ok(Response::Buried(None)),
            "TOUCHED" => Ok(Response::Touched),
            "RELEASED" => Ok(Response::Released),
            "DELETED" => Ok(Response::Deleted),
            "KICKED" => Ok(Response::JobKicked),
            _ => Err(BeanError::Parsing(ParsingError::UnknownResponse))?,
        };

        trace!(?result, "processed single word response");
        result
    }

    /// Helper method which handles all two word responses
    #[instrument]
    fn two_word_response(&self, list: &[&str]) -> Result<Response, BeanError> {
        let result = match list[0] {
            "INSERTED" => {
                let id: u32 = u32::from_str(list[1])
                    .map_err(|_| BeanError::Parsing(ParsingError::ParseId))?;
                Ok(Response::Inserted(id))
            }
            "BURIED" => {
                let id: u32 = u32::from_str(list[1])
                    .map_err(|_| BeanError::Parsing(ParsingError::ParseId))?;
                Ok(Response::Buried(Some(id)))
            }
            "WATCHING" => {
                let count = u32::from_str(list[1])
                    .map_err(|_| BeanError::Parsing(ParsingError::ParseId))?;
                Ok(Response::Watching(count))
            }
            "USING" => Ok(Response::Using(String::from(list[1]))),
            "KICKED" => {
                let count: u32 = u32::from_str(list[1])
                    .map_err(|_| BeanError::Parsing(ParsingError::ParseNumber))?;
                Ok(Response::Kicked(count))
            }
            _ => Err(BeanError::Parsing(ParsingError::UnknownResponse))?,
        };

        trace!(?result, "Parsed two word response");
        result
    }

    #[instrument]
    fn parse_response(&self, list: &[&str]) -> Result<Response, BeanError> {
        debug!("Parsing response");
        if list.len() == 1 {
            return self.single_word_response(list);
        }

        if list.len() == 2 {
            eprintln!("Parsing: {:?}", list[1]);
            return self.two_word_response(list);
        }

        if list.len() == 3 {
            return match list[0] {
                "RESERVED" => Ok(Response::Pre(parse_pre_job(
                    &list[1..],
                    response::PreResponse::Reserved,
                )?)),
                "FOUND" => Ok(Response::Pre(parse_pre_job(
                    &list[1..],
                    response::PreResponse::Peek,
                )?)),
                _ => Err(BeanError::Parsing(ParsingError::UnknownResponse))?,
            };
        }

        Err(BeanError::Parsing(ParsingError::UnknownResponse))?
    }

    #[instrument(skip_all)]
    fn parse_job(&mut self, src: &mut BytesMut, pre: &PreJob) -> Result<Option<Job>, BeanError> {
        if let Some(carriage_offset) = src.iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                let line = utf8(src).map_err(|_| BeanError::Parsing(ParsingError::ParseString))?;
                let line: Vec<&str> = line.trim().split(' ').collect();

                let job = Job {
                    id: pre.id,
                    bytes: pre.bytes,
                    data: line[0].as_bytes().to_vec(),
                };

                trace!(?job);
                return Ok(Some(job));
            }
        }
        self.outstart += src.len();
        Ok(None)
    }

    #[instrument(skip(src))]
    fn handle_job_response(
        &mut self,
        response: Response,
        src: &mut BytesMut,
    ) -> Result<Option<Response>, BeanError> {
        if let Response::Pre(pre) = response {
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

fn parse_pre_job(list: &[&str], response_type: response::PreResponse) -> Result<PreJob, BeanError> {
    let id = u32::from_str(list[0]).map_err(|_| BeanError::Parsing(ParsingError::ParseId))?;
    let bytes = usize::from_str(list[1]).map_err(|_| BeanError::Parsing(ParsingError::ParseId))?;
    Ok(PreJob {
        id,
        bytes,
        response_type,
    })
}

impl Decoder for CommandCodec {
    type Item = Response;
    type Error = BeanError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        trace!(source = ?src , "Decoding");
        if let Some(carriage_offset) = src[self.outstart..].iter().position(|b| *b == b'\r') {
            if src[carriage_offset + 1] == b'\n' {
                // Afterwards src contains elements [at, len), and the returned BytesMut
                // contains elements [0, at), so + 1 for \r and then +1 for \n
                let offset = self.outstart + carriage_offset + 1 + 1;
                let line = src.split_to(offset);
                let line =
                    utf8(&line).map_err(|_| BeanError::Parsing(ParsingError::ParseString))?;
                let line: Vec<&str> = line.trim().split(' ').collect();

                let response = self.parse_response(&line[..])?;
                debug!(?response, "Got response");

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

impl Encoder<Request> for CommandCodec {
    type Error = EncodeError;

    #[instrument(skip_all)]
    fn encode(&mut self, item: Request, dst: &mut BytesMut) -> Result<(), Self::Error> {
        trace!(?item, "Making request");
        match item {
            Request::Watch { tube } => {
                if tube.as_bytes().len() > 200 {
                    return Err(EncodeError::TubeNameTooLong);
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
