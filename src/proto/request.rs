#[derive(Debug)]
pub(crate) enum Request {
    Put {
        priority: u32,
        delay: u32,
        ttr: u32,
        data: &'static str,
    },
    Reserve,
}
