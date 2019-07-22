# async-stream

[![Apache-2.0 licensed](https://img.shields.io/badge/license-Apache2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0.txt)
[![crates.io](https://meritbadge.herokuapp.com/async-stream)](https://crates.io/crates/async-stream)
[![Released API docs](https://docs.rs/async-stream/badge.svg)](https://docs.rs/async-stream)

### Use an async closure to produce items for a stream.

A way to produce items for a stream from an [async closure][async_block], while we
are waiting for official [async yield][async_yield] support.

The obvious solution would be to just use a channel, spawn the
async closure as a seperate task, letting it send items onto the
sink end of the channel - the other end is the stream, producing
those items.

However this results in overhead - memory allocations, context switching
between the tasks, unnecessary buffering, and it makes canceling
the stream (dropping it) harder than it needs to be.

The AsyncStream from this crate runs as a stream in the current task,
using only async/await and no other unstable features.

Example:

```rust
#![feature(async_await)]
use futures::StreamExt;
use futures::executor::block_on;
use async_stream::async_stream;

let mut strm = async_stream!(u8, {
    for i in 0u8..10 {
        stream_send!(i);
    }
});

let fut = async {
    let mut count = 0;
    while let Some(item) = strm.next().await {
        println!("{:?}", item);
        count += 1;
    }
    assert!(count == 10);
};
block_on(fut);

```
[async_block]: https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#async-blocks-vs-async-closures
[async_yield]: https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#generators-and-streams


### Copyright and License.

 * © 2019 Miquel van Smoorenburg
 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
