use std::task::Poll::*;

/// Item Type for auto_stream_select function.
#[derive(Debug)]
pub enum AutoStreamSelect<LeftType, RightType> {
    /// The left-hand stream output.
    Left(Option<LeftType>),

    /// The right-hand stream output.
    Right(Option<RightType>),
}

/// Merge two sub-streams so they can be polled in parallel, but
/// still know when each individually ends, unlike futures::stream::select()
pub fn auto_stream_select<LeftType, RightType, LSt, RSt>(
    left: LSt,
    right: RSt,
) -> impl futures::stream::Stream<Item = AutoStreamSelect<LeftType, RightType>>
where
    LSt: futures::stream::Stream<Item = LeftType> + Unpin,
    RSt: futures::stream::Stream<Item = RightType> + Unpin,
{
    let mut left = Some(left);
    let mut l_done = false;
    let left = futures::stream::poll_fn(move |ctx| {
        if l_done {
            return Ready(None);
        }
        let rleft = left.as_mut().unwrap();
        tokio::pin!(rleft);
        match futures::stream::Stream::poll_next(rleft, ctx) {
            Ready(Some(v)) => Ready(Some(AutoStreamSelect::Left(Some(v)))),
            Ready(None) => {
                l_done = true;
                drop(left.take().unwrap());
                Ready(Some(AutoStreamSelect::Left(None)))
            }
            Pending => Pending,
        }
    });

    let mut right = Some(right);
    let mut r_done = false;
    let right = futures::stream::poll_fn(move |ctx| {
        if r_done {
            return Ready(None);
        }
        let rright = right.as_mut().unwrap();
        tokio::pin!(rright);
        match futures::stream::Stream::poll_next(rright, ctx) {
            Ready(Some(v)) => Ready(Some(AutoStreamSelect::Right(Some(v)))),
            Ready(None) => {
                r_done = true;
                drop(right.take().unwrap());
                Ready(Some(AutoStreamSelect::Right(None)))
            }
            Pending => Pending,
        }
    });

    futures::stream::select(left, right)
}
