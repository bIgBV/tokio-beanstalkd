use bytes::{BufMut, BytesMut};
use std::borrow::Cow;
use std::fmt;

#[derive(Debug)]
pub enum Request {
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
}

impl Request {
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
            Request::Reserve => {}
            Request::Use { .. } => {}
        }
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
        }
    }
}
