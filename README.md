# tokio-beanstalkd

This crate provides a client for working with [Beanstalkd](https://beanstalkd.github.io/), a simple
fast work queue.

[![Build Status](https://travis-ci.org/bIgBV/tokio-beanstalkd.svg?branch=master)](https://travis-ci.org/bIgBV/tokio-beanstalkd)
[![Crates.io](https://img.shields.io/crates/v/tokio-beanstalkd.svg)](https://crates.io/crates/tokio-beanstalkd)
[![Documentation](https://docs.rs/tokio-beanstalkd/badge.svg)](https://docs.rs/tokio-beanstalkd/)

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

```rust
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
             bean.put(0, 1, 100, &b"data"[..])
                 .inspect(|(_, response)| {
                     response.as_ref().unwrap();
                 }).and_then(|(bean, _)| bean.reserve())
                 .inspect(|(_, response)| assert_eq!(response.as_ref().unwrap().data, b"data"))
                 .and_then(|(bean, response)| bean.touch(response.unwrap().id))
                 .inspect(|(_, response)| {
                     response.as_ref().unwrap();
                 }).and_then(|(bean, _)| {
                     // how about another one?
                     bean.put(0, 1, 100, &b"more data"[..])
                 }).and_then(|(bean, _)| bean.reserve())
                 .and_then(|(bean, response)| bean.release(response.unwrap().id, 10, 10))
                 .inspect(|(_, response)| {
                     response.as_ref().unwrap();
                 }).and_then(|(bean, _)| bean.reserve())
                 .and_then(|(bean, response)| bean.bury(response.unwrap().id, 10))
                 .inspect(|(_, response)| {
                     response.as_ref().unwrap();
                 }).and_then(|(bean, _)| {
                     // how about another one?
                     bean.put(0, 1, 100, &b"more data"[..])
                 }).inspect(|(_, response)| {
                     response.as_ref().unwrap();
                 }).and_then(|(bean, response)| bean.delete(response.unwrap()))
                 .inspect(|(_, response)| {
                     // assert_eq!(*e, error::Consumer::NotFound);
                     response.as_ref().unwrap();
                 }).and_then(|(bean, _)| bean.watch("test"))
                 .inspect(|(_, response)| assert_eq!(*response.as_ref().unwrap(), 2))
                 .and_then(|(bean, _)| bean.ignore("test"))
                 .inspect(|(_, response)| {
                     assert_eq!(response.as_ref().unwrap(), &IgnoreResponse::Watching(1))
                 })
         }),
     );
     assert!(!bean.is_err());
     drop(bean);
     rt.shutdown_on_idle();
 }
```
