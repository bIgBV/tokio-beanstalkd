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
have to `delete` job. This is required for every job, or else Beanstalkd will not remove it from
its internal data structures.


If a worker cannot finish the job in it's TTR (Time To Run), then it can `release` the job. The
application can use the `using` method to put jobs in a specific tube, and workers can use `watch`
to only reserve jobs from the specified tubes.

## Interaction with Tokio

The futures in this crate expect to be running under a `tokio::Runtime`. In the common case,
you cannot resolve them solely using `.wait()`, but should instead use `tokio::run` or
explicitly create a `tokio::Runtime` and then use `Runtime::block_on`.

An simple example client could look something like this:

```rust
let mut rt = tokio::runtime::Runtime::new().unwrap();
let bean = rt.block_on(
    Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
        bean.put(0, 1, 100, &b"update:42"[..])
            .inspect(|(_, response)| {
                response.as_ref().unwrap();
            })
            .and_then(|(bean, _)| {
                // Use a particular tube
                bean.using("notifications")
            }).and_then(|(bean, _)| bean.put(0, 1, 100, &b"notify:100"[..]))
    }),
);
```

And a worker could look something like this:
```rust
 let mut rt = tokio::runtime::Runtime::new().unwrap();
 let bean = rt.block_on(
     Beanstalkd::connect(&"127.0.0.1:11300".parse().unwrap()).and_then(|bean| {
         bean.reserve()
             .inspect(|(_, response)| {
                 // Do something with the response
             }).and_then(|(bean, response)| {
                 // Delete the job once it is done
                 bean.delete(response.as_ref().unwrap().id)
             })
     }),
 );
 rt.shutdown_on_idle();
```
