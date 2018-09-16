# tokio-beanstalkd

This crate provides a client for working with [Beanstalkd](https://beanstalkd.github.io/), a simple
fast work queue.

# About Beanstalkd

Beanstalkd is a simple fast work queue. It works at the TCP connection level, considering each TCP
connection individually. A worker may have multiple connections to the Beanstalkd server and each
connection will be considered separate.

The protocol is ASCII text based but the data itself is just a bytestream. This means that the
application is responsible for interpreting the data.

## Operation
This library can serve as a client for both the application and the worker. The application would
`put` jobs on the queue and the workers can `reserve` them. Once they are done with the job, they
have to `delete` job. This is required for every job, or else beanstlkd will not remove it from
its internal datastructres.

If a worker cannot finish the job in it's TTR (Time To Run), then it can `release` the job. The
application can use the `using` method to put jobs in a specific tube, and workers can use `watch`
to only reserve jobs from the specified tubes.

## Interaction with Tokio

The futures in this crate expect to be running under a `tokio::Runtime`. In the common case,
you cannot resolve them solely using `.wait()`, but should instead use `tokio::run` or
explicitly create a `tokio::Runtime` and then use `Runtime::block_on`.

A contrived example

```no_run
extern crate tokio;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate tokio_beanstalkd;

use tokio::prelude::*;
use tokio_beanstalkd::*;

 fn consumer_commands() {
     let mut rt = tokio::runtime::Runtime::new().unwrap();
     let bean = rt.block_on(
         Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
             bean.put(0, 1, 1, &b"data"[..])
                 .inspect(|(_, response)| assert!(response.is_ok()))
                 .and_then(|(bean, _)| bean.reserve())
                 .inspect(|(_, response)| match response {
                     Ok(Response::Reserved(j)) => {
                         assert_eq!(j.data, b"data");
                     }
                     _ => panic!("Wrong response received"),
                 })
                 .and_then(|(bean, response)| match response {
                     Ok(Response::Reserved(j)) => bean.touch(j.id),
                     Ok(_) => panic!("Wrong response returned"),
                     Err(e) => panic!("Got error: {}", e),
                 })
                 .inspect(|(_, response)| match response {
                     Ok(v) => assert_eq!(*v, Response::Touched),
                     Err(e) => panic!("Got error: {}", e),
                 })
                 .and_then(|(bean, _)| {
                     // how about another one?
                     bean.put(0, 1, 1, &b"more data"[..])
                 })
                 .and_then(|(bean, _)| bean.reserve())
                 .and_then(|(bean, response)| match response {
                     Ok(Response::Reserved(job)) => bean.release(job.id, 10, 10),
                     Ok(_) => panic!("Wrong response returned"),
                     Err(e) => panic!("Got error: {}", e),
                 })
                 .inspect(|(_, response)| match response {
                     Ok(v) => assert_eq!(*v, Response::Released),
                     Err(e) => panic!("Got error: {}", e),
                 })
                 .and_then(|(bean, _)| bean.reserve())
                 .and_then(|(bean, response)| match response {
                     Ok(Response::Reserved(job)) => bean.bury(job.id, 10),
                     Ok(_) => panic!("Wrong response returned"),
                     Err(e) => panic!("Got error: {}", e),
                 })
                 .inspect(|(_, response)| match response {
                     Ok(v) => assert_eq!(*v, Response::Buried),
                     Err(e) => panic!("Got error: {}", e),
                 })
                 .and_then(|(bean, _)| {
                     // how about another one?
                     bean.put(0, 1, 1, &b"more data"[..])
                 })
                 .inspect(|(_, response)| assert!(response.is_ok()))
                 .and_then(|(bean, response)| match response {
                     Ok(Response::Inserted(id)) => bean.delete(id),
                     Ok(_) => panic!("Wrong response returned"),
                     Err(e) => panic!("Got error: {}", e),
                 })
                 .inspect(|(_, response)| match response {
                     Ok(v) => assert_eq!(*v, Response::Deleted),
                     Err(e) => {
                         // assert_eq!(*e, error::Consumer::NotFound);
                         panic!("Got error: {}", e)
                     }
                 })
                 .and_then(|(bean, _)| bean.watch("test"))
                 .inspect(|(_, response)| match response {
                     Ok(v) => assert_eq!(*v, Response::Watching(2)),
                     Err(e) => panic!("Got error: {}", e),
                 })
                 .and_then(|(bean, _)| bean.ignore("test"))
                 .inspect(|(_, response)| match response {
                     Ok(v) => assert_eq!(*v, Response::Watching(1)),
                     Err(e) => panic!("Got error: {}", e),
                 })
         }),
     );
     assert!(!bean.is_err());
     drop(bean);
     rt.shutdown_on_idle();
 }
```
