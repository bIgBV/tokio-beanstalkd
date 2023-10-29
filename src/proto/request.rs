use std::borrow::Cow;
use std::fmt;

use tokio_util::bytes::{BufMut, BytesMut};

/// A Request holds the data to be serialized on to the wire.
#[derive(Debug)]
pub(crate) enum Request {
    Put {
        priority: u32,
        delay: u32,
        ttr: u32,
        data: Cow<'static, [u8]>,
    },
    Reserve,
    Use {
        tube: &'static str,
    },
    Delete {
        id: super::Id,
    },
    Release {
        id: super::Id,
        priority: u32,
        delay: u32,
    },
    Bury {
        id: super::Id,
        priority: u32,
    },
    Touch {
        id: super::Id,
    },
    Watch {
        tube: &'static str,
    },
    Ignore {
        tube: &'static str,
    },
    Peek {
        id: super::Id,
    },
    PeekReady,
    PeekDelay,
    PeekBuried,
    Kick {
        bound: u32, // Can this and other u32 values be 64 bits?
    },
    KickJob {
        id: super::Id,
    },
}

impl Request {
    /// The serailize method is used to serialize any request containig data.
    /// Since the beanstalkd protocol consists af ASCII characters, the
    /// actual heavy lifting is done by the `Display` implementation.
    pub(crate) fn serialize(&self, dst: &mut BytesMut) {
        let format_string = format!("{}", &self);
        dst.reserve(format_string.len());
        dst.put(format_string.as_bytes());
        match *self {
            Request::Put { ref data, .. } => {
                dst.reserve(data.len());
                dst.put(&data[..]);
                dst.reserve(2);
                dst.put(&b"\r\n"[..]);
            }
            Request::Reserve
            | Request::PeekReady
            | Request::PeekDelay
            | Request::PeekBuried
            | Request::Use { .. }
            | Request::Delete { .. }
            | Request::Release { .. }
            | Request::Bury { .. }
            | Request::Touch { .. }
            | Request::Watch { .. }
            | Request::Ignore { .. }
            | Request::Peek { .. }
            | Request::Kick { .. }
            | Request::KickJob { .. } => {}
        }
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Request::Put {
                priority,
                delay,
                ttr,
                ref data,
            } => write!(
                f,
                "put {pri} {del} {ttr} {len}\r\n",
                pri = priority,
                del = delay,
                ttr = ttr,
                len = data.len(),
            ),
            Request::Reserve => write!(f, "reserve\r\n"),
            Request::Use { tube } => write!(f, "use {}\r\n", tube),
            Request::Delete { id } => write!(f, "delete {}\r\n", id),
            Request::Release {
                id,
                priority,
                delay,
            } => write!(f, "release {} {} {}\r\n", id, priority, delay),
            Request::Bury { id, priority } => write!(f, "bury {} {}\r\n", id, priority),
            Request::Touch { id } => write!(f, "touch {}\r\n", id),
            Request::Watch { tube } => write!(f, "watch {}\r\n", tube),
            Request::Ignore { tube } => write!(f, "ignore {}\r\n", tube),
            Request::Peek { id } => write!(f, "peek {}\r\n", id),
            Request::PeekReady => write!(f, "peek-ready\r\n"),
            Request::PeekDelay => write!(f, "peek-delayed\r\n"),
            Request::PeekBuried => write!(f, "peek-buried\r\n"),
            Request::Kick { bound } => write!(f, "kick {}\r\n", bound),
            Request::KickJob { id } => write!(f, "kick-job {}\r\n", id),
        }
    }
}
