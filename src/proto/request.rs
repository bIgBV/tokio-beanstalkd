use std::fmt;

#[derive(Debug)]
pub enum Request {
    Put {
        priority: u32,
        delay: u32,
        ttr: u32,
        data: &'static str,
    },
    Reserve,
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Request::Put {
                priority,
                delay,
                ttr,
                data,
            } => write!(
                f,
                "put {pri} {del} {ttr} {len}\r\n{data}\r\n",
                pri = priority,
                del = delay,
                ttr = ttr,
                len = data.len(),
                data = data
            ),
            Request::Reserve => write!(f, "reserve\r\n"),
        }
    }
}
