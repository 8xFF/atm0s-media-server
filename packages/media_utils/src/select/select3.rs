#[doc(no_inline)]
pub use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pub enum OrOutput<T1, T2, T3> {
    Left(T1),
    Middle(T2),
    Right(T3),
}

pub fn or<T1, T2, T3, F1, F2, F3>(future1: F1, future2: F2, future3: F3) -> Or<F1, F2, F3>
where
    F1: Future<Output = T1>,
    F2: Future<Output = T2>,
    F3: Future<Output = T3>,
{
    Or { future1, future2, future3 }
}

pin_project! {
    /// Future for the [`or()`] function and the [`FutureExt::or()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Or<F1, F2, F3> {
        #[pin]
        future1: F1,
        #[pin]
        future2: F2,
        #[pin]
        future3: F3,
    }
}

impl<T1, T2, T3, F1, F2, F3> Future for Or<F1, F2, F3>
where
    F1: Future<Output = T1>,
    F2: Future<Output = T2>,
    F3: Future<Output = T3>,
{
    type Output = OrOutput<T1, T2, T3>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if let Poll::Ready(t) = this.future1.poll(cx) {
            return Poll::Ready(OrOutput::Left(t));
        }
        if let Poll::Ready(t) = this.future2.poll(cx) {
            return Poll::Ready(OrOutput::Middle(t));
        }
        if let Poll::Ready(t) = this.future3.poll(cx) {
            return Poll::Ready(OrOutput::Right(t));
        }
        Poll::Pending
    }
}
