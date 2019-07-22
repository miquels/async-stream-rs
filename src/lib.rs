#![deny(unused_must_use)]
#![doc(html_root_url = "https://docs.rs/async-stream/0.1.0-alpha.1")]
//! ## Use an async closure to produce items for a stream.
//!
//! A way to produce items for a stream from an [async closure][async_block], while we
//! are waiting for official [async yield][async_yield] support.
//!
//! The obvious solution would be to just use a channel, spawn the
//! async closure as a seperate task, letting it send items onto the
//! sink end of the channel - the other end is the stream, producing
//! those items.
//!
//! However this results in overhead - memory allocations, context switching
//! between the tasks, unnecessary buffering, and it makes canceling
//! the stream (dropping it) harder than it needs to be.
//!
//! The AsyncStream from this crate runs as a stream in the current task,
//! using only async/await and no other unstable features.
//!
//! Example:
//!
//! ```
//! #![feature(async_await)]
//! use futures::StreamExt;
//! use futures::executor::block_on;
//! use async_stream::async_stream;
//!
//! let mut strm = async_stream!(u8, {
//!     for i in 0u8..10 {
//!         stream_send!(i);
//!     }
//! });
//!
//! let fut = async {
//!     let mut count = 0;
//!     while let Some(item) = strm.next().await {
//!         println!("{:?}", item);
//!         count += 1;
//!     }
//!     assert!(count == 10);
//! };
//! block_on(fut);
//!
//! ```
//! [async_block]: https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#async-blocks-vs-async-closures
//! [async_yield]: https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#generators-and-streams
//!
use std::cell::Cell;
use std::pin::Pin;
use std::sync::Arc;

use futures::task::{Context, Poll};
use futures::{Future, Stream};

/// Convenience macro to create an `AsyncStream`.
///
/// The macro is called as `async_stream!(_item_, { .. _code_ ..})`, which then expands to:
/// ```rust ignore
/// AsyncStream::<item>::new(|mut sender| async move { .. code .. })
/// ```
/// Within the code block you can use the `stream_send!(_item_)` macro, which expands to:
/// ```rust ignore
/// sender.send(item).await
/// ```
///
/// Adding `move`, as in `async_stream!(_item_, move { .. _code_ ..})`, does
/// what you expect it to do.
#[macro_export]
macro_rules! async_stream {
    ($item_type:ty, $code:tt) => {
        $crate::AsyncStream::<$item_type>::new(|mut sender| {
            async move {
                macro_rules! stream_send (
                        ($item:expr) => {
                            sender.send($item).await
                        };
                    );
                $code
            }
        })
    };
    ($item_type:ty, move $code:tt) => {
        $crate::AsyncStream::<$item_type>::new(move |mut sender| {
            async move {
                macro_rules! stream_send (
                        ($item:expr) => {
                            sender.send($item).await
                        };
                    );
                $code
            }
        })
    };
}

/// Convenience macro to create an `AsyncTryStream`.
///
/// The same as `async_stream!` but creates an `AsyncTryStream`.
#[macro_export]
macro_rules! async_try_stream {
    ($item_type:ty, $error_type:ty, $code:tt) => {
        $crate::AsyncTryStream::<$item_type, $error_type>::new(|mut sender| {
            async move {
                macro_rules! stream_send (
                        ($item:expr) => {
                            sender.send($item).await
                        };
                    );
                $code
            }
        })
    };
    ($item_type:ty, $error_type:ty, move $code:tt) => {
        $crate::AsyncTryStream::<$item_type, $error_type>::new(move |mut sender| {
            async move {
                macro_rules! stream_send (
                        ($item:expr) => {
                            sender.send($item).await
                        };
                    );
                $code
            }
        })
    };
}

/// Future returned by the Sender.send() method.
///
/// Completes when the item is sent. _Must_ be `await`ed.
#[must_use = "return value of `async_stream::Sender::&lt;I&gt;::send()` must be awaited"]
pub struct SenderFuture {
    is_ready: bool,
}

impl SenderFuture {
    // constructor. private.
    fn new() -> SenderFuture {
        SenderFuture { is_ready: false }
    }
}

impl Future for SenderFuture {
    type Output = ();

    // This poll function returns Pending once, then the
    // next time it is called it returns Ready.
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.is_ready {
            Poll::Ready(())
        } else {
            self.is_ready = true;
            Poll::Pending
        }
    }
}

// Only internally used by one AsyncStream and never shared
// in any other way, so we don't have to use Arc<Mutex<..>>.
/// Type of the sender passed as first argument into the async closure.
pub struct Sender<I>(Arc<Cell<Option<I>>>);
unsafe impl<I> Sync for Sender<I> {}
unsafe impl<I> Send for Sender<I> {}

impl<I> Sender<I> {
    fn new() -> Sender<I> {
        Sender(Arc::new(Cell::new(None)))
    }

    // note that this is NOT impl Clone for Sender, it's private.
    fn clone(&self) -> Sender<I> {
        Sender(Arc::clone(&self.0))
    }

    /// Send one item to the stream.
    pub fn send<T>(&mut self, item: T) -> SenderFuture
    where T: Into<I> {
        self.0.set(Some(item.into()));
        SenderFuture::new()
    }
}

/// Produce items for a stream from an async closure.
///
/// `AsyncStream::new()` takes a [Future][Future03] ([async closure][async_closure], usually),
/// and returns an `AsyncStream` that implements a [Stream][Stream03].
///
/// Async closures are not stabilised yet, but you can wrap an async
/// block in a closure which is very similar, as [documented in the async/await RFC][async_block].
///
/// Example:
///
/// ```ignore rust
/// let mut strm = AsyncStream::<u8>::new(|sender| async move {
///     for i in 0u8..10 {
///         sender.send(i).await;
///     }
/// });
/// ```
/// [async_closure]: https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#async--closures
/// [async_block]: https://github.com/rust-lang/rfcs/blob/master/text/2394-async_await.md#async-blocks-vs-async-closures
/// [Future03]: https://doc.rust-lang.org/nightly/std/future/trait.Future.html
/// [Stream03]: https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.17/futures/stream/trait.Stream.html
#[must_use]
pub struct AsyncStream<Item> {
    item: Sender<Item>,
    fut:  Option<Pin<Box<dyn Future<Output = ()> + 'static + Send>>>,
}

impl<Item> AsyncStream<Item> {
    /// Create a new Stream from an async closure.
    ///
    /// The closure is passed one argument, the sender, which has a
    /// method "send" that can be called to send a item to the stream.
    pub fn new<F, R>(f: F) -> Self
    where
        F: FnOnce(Sender<Item>) -> R,
        R: Future<Output = ()> + Send + 'static,
        Item: 'static,
    {
        let sender = Sender::new();
        AsyncStream::<Item> {
            fut:  Some(Box::pin(f(sender.clone()))),
            item: sender,
        }
    }
}

/// Stream implementation for futures 0.3.
impl<I> Stream for AsyncStream<I> {
    type Item = I;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<I>> {
        let pollres = {
            let fut = self.fut.as_mut().unwrap();
            fut.as_mut().poll(cx)
        };
        match pollres {
            // If the future returned Poll::Ready, that signals the end of the stream.
            Poll::Ready(_) => Poll::Ready(None),
            Poll::Pending => {
                // Pending means that some sub-future returned pending. That sub-future
                // _might_ have been the SenderFuture returned by Sender.send, so
                // check if there is an item available in self.item.
                let mut item = self.item.0.replace(None);
                if item.is_none() {
                    Poll::Pending
                } else {
                    Poll::Ready(Some(item.take().unwrap()))
                }
            },
        }
    }
}

/// Like `AsyncStream`, but produces `Result` values.
///
/// This is like `AsyncStream`, but the async closure returns a `Result` instead of `()`.
/// If it returns an `Err`, that error value is returned as an error on the stream.
/// For a normal end-of-stream it must return `Ok(())`.
///
/// This means you can use idiomatic error handling with `?` etcetera.
///
/// If the `compat` feature-flag is set, `AsyncTryStream` will also implement
/// the [futures 0.1 Stream trait][Stream01].
///
/// [Stream01]: https://docs.rs/futures/0.1.28/futures/stream/trait.Stream.html
///
#[must_use]
pub struct AsyncTryStream<Item, Error> {
    item: Sender<Item>,
    fut:  Option<Pin<Box<dyn Future<Output = Result<(), Error>> + 'static + Send>>>,
}

impl<Item, Error: 'static + Send> AsyncTryStream<Item, Error> {
    /// Create a new AsyncTryStream from an async closure.
    ///
    /// The closure is passed one argument, the sender, which has a
    /// method "send" that can be called to send a item to the stream.
    pub fn new<F, R>(f: F) -> Self
    where
        F: FnOnce(Sender<Item>) -> R,
        R: Future<Output = Result<(), Error>> + Send + 'static,
        Item: 'static,
    {
        let sender = Sender::new();
        AsyncTryStream::<Item, Error> {
            fut:  Some(Box::pin(f(sender.clone()))),
            item: sender,
        }
    }
}

/// Stream implementation for Futures 0.3.
impl<I, E> Stream for AsyncTryStream<I, E> {
    type Item = Result<I, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<I, E>>> {
        let pollres = {
            let fut = self.fut.as_mut().unwrap();
            fut.as_mut().poll(cx)
        };
        match pollres {
            // If the future returned Poll::Ready, that signals the end of the stream.
            Poll::Ready(Ok(_)) => Poll::Ready(None),
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => {
                // Pending means that some sub-future returned pending. That sub-future
                // _might_ have been the SenderFuture returned by Sender.send, so
                // check if there is an item available in self.item.
                let mut item = self.item.0.replace(None);
                if item.is_none() {
                    Poll::Pending
                } else {
                    Poll::Ready(Some(Ok(item.take().unwrap())))
                }
            },
        }
    }
}

#[cfg(feature = "compat")]
mod stream01 {
    use futures::compat::Compat as Compat03As01;
    use futures01::Async as Async01;
    use futures01::Future as Future01;
    use futures01::Stream as Stream01;

    /// Stream implementation for Futures 0.1.
    impl<I, E> Stream01 for crate::AsyncTryStream<I, E> {
        type Item = I;
        type Error = E;

        fn poll(&mut self) -> Result<Async01<Option<Self::Item>>, Self::Error> {
            // We use a futures::compat::Compat wrapper to be able to call
            // the futures 0.3 Future in a futures 0.1 context. Because
            // the Compat wrapper wants to to take ownership, the future
            // is stored in an Option which we can temporarily move it out
            // of, and then move it back in.
            let mut fut = Compat03As01::new(self.fut.take().unwrap());
            let pollres = fut.poll();
            self.fut.replace(fut.into_inner());
            match pollres {
                Ok(Async01::Ready(_)) => Ok(Async01::Ready(None)),
                Ok(Async01::NotReady) => {
                    let mut item = self.item.0.replace(None);
                    if item.is_none() {
                        Ok(Async01::NotReady)
                    } else {
                        Ok(Async01::Ready(item.take()))
                    }
                },
                Err(e) => Err(e),
            }
        }
    }
}

#[cfg(feature = "compat")]
#[doc(inline)]
pub use stream01::*;
