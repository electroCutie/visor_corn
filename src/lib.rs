/// Wrap Futures so that the result of their computation is available in non-async functions
/// by blocking waiting for the result

use std::{future::Future, sync::mpsc::{RecvError, SendError}};
use promissory::{Awaiter, Fulfiller};

pub enum FutureResult<T>
where
    T: Send,
{
    Waiting(Awaiter<T>),
    Purgatory,
    Ready(T),
}

impl<T: Send> FutureResult<T> {
    pub fn await_value<'a>(&'a mut self) -> Result<&'a mut T, RecvError> {
        match self {
            FutureResult::Purgatory => unreachable!(),
            FutureResult::Ready(t) => Ok(t),
            FutureResult::Waiting(_) => {
                let mut owned = FutureResult::Purgatory;
                std::mem::swap(&mut owned, self);

                if let FutureResult::Waiting(awaiter) = owned {
                    let t = awaiter.await_value()?;
                    *self = FutureResult::Ready(t);
                    if let FutureResult::Ready(t) = self{
                        Ok(t)
                    }else{
                        unreachable!(/* We just set it */)
                    }
                }else{
                    unreachable!(/* We just swapped owned and self */)
                }
            }
        }
    }

    pub fn await_take(self) -> Result<T, RecvError> {
        match self {
            FutureResult::Purgatory => unreachable!(),
            FutureResult::Ready(t) => Ok(t),
            FutureResult::Waiting(awaiter) => awaiter.await_value()
        }
    }
}



// async lambdas are not yet stable
async fn asynchronize<T, F, H, R>(
    fulfiller: Fulfiller<T>,
    to_wrap: F,
    handler: H
) -> R
where
    T: Send,
    F: Future<Output = T>,
    H: FnOnce(Result<(), SendError<T>>) -> R,
{
    handler(fulfiller.fulfill(to_wrap.await))
}

/// Wrap a Future and provide an object to retrieve the result of the Future
pub fn waitable_future<T, F>(
    to_wrap: F,
) -> (
    FutureResult<T>,
    impl Future<Output = Result<(), SendError<T>>>,
)
where
    T: Send,
    F: Future<Output = T>,
{
    let (fulfiller, awaiter) = promissory::promissory();
    (
        FutureResult::Waiting(awaiter),
        asynchronize(fulfiller, to_wrap, |x| x ),
    )
}

/// Wrap a Future and provide an object to retrieve the result of the Future, panicing if the wsend fails
/// The send could normally only fail if the the FutureResult is dropped before the Future completes
pub fn waitable_future_expect<T, F>(
    to_wrap: F, msg: &'static str,
) -> ( FutureResult<T>, impl Future<Output = ()>)
where
    T: Send,
    F: Future<Output = T>,
{
    let (fulfiller, awaiter) = promissory::promissory();
    (
        FutureResult::Waiting(awaiter),
        asynchronize(fulfiller, to_wrap, move |x| x.expect(msg) ),
    )
}

#[cfg(test)]
mod tests {
    async fn async_buddy() -> u64{
        std::thread::sleep(std::time::Duration::from_millis(100));
        14
    }

    #[test]
    fn it_works() {
        let (result, future) = 
            super::waitable_future(async_buddy());

        smol::spawn(future).detach();

        let mut result = result;
        assert_eq!(&14, result.await_value().expect("msg problem"));
        assert_eq!(&14, result.await_value().expect("msg problem 2"));

        assert_eq!(14, result.await_take().expect("msg problem 3"));
    }
}
