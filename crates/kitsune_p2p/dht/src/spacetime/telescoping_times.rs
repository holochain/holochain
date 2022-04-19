use super::*;

/// A type which generates a list of exponentially expanding time windows
/// which fit into a tree structure. See [this document](https://hackmd.io/@hololtd/r1IAIbr5Y)
/// for the full understanding.
///
/// TODO: add this documentation to the codebase
#[derive(Copy, Clone, Debug, PartialEq, Eq, Derivative, serde::Serialize, serde::Deserialize)]
#[derivative(PartialOrd, Ord)]
pub struct TelescopingTimes {
    time: TimeQuantum,

    #[derivative(PartialOrd = "ignore")]
    #[derivative(Ord = "ignore")]
    limit: Option<u32>,
}

impl TelescopingTimes {
    /// An empty set of times
    pub fn empty() -> Self {
        Self {
            time: 0.into(),
            limit: None,
        }
    }

    /// Constructor,
    pub fn new(time: TimeQuantum) -> Self {
        Self { time, limit: None }
    }

    /// Get TelescopingTimes from the origin time up until times less than
    /// `recent_threshold` ago, to be handled by historical gossip.
    /// (Recent gossip will handle everything after the threshold.)
    pub fn historical(topo: &Topology, recent_threshold: Duration) -> Self {
        let threshold = (Timestamp::now() - recent_threshold)
            .expect("The system time is set to something unreasonable");
        let time_quantum = TimeQuantum::from_timestamp(topo, threshold);
        // Add 1 quantum to "round up", so that the final time window includes
        // the threshold
        Self::new(time_quantum + 1.into())
    }

    /// Calculate the exponentially expanding time segments using the binary
    /// representation of the current timestamp.
    ///
    /// The intuition for this algorithm is that the position of the most
    /// significant 1 represents the power of the largest, leftmost time segment,
    /// and subsequent bits represent the powers of 2 below that one.
    /// After the MSB, a 0 represents a single value of the power represented
    /// by that bit, and a 1 represents two values of the power at that bit.
    ///
    /// See the test below which has the first 16 time segments, each alongside
    /// the binary representation of the timestamp (+1) which generated it.
    /// Seeing the pattern in that test is the best way to understand this.
    pub fn segments(&self) -> Vec<TimeSegment> {
        let mut now: u32 = self.time.inner() + 1;
        if now == 1 {
            return vec![];
        }
        let zs = now.leading_zeros() as u8;
        now <<= zs;
        let iters = 32 - zs - 1;
        let mut max = self.limit.unwrap_or(u32::from(iters) * 2);
        if max == 0 {
            return vec![];
        }
        let mut seg = TimeSegment::new(iters, 0);
        let mut times = vec![];
        let mask = 1u32.rotate_right(1); // 0b100000...
        for _ in 0..iters {
            seg.power -= 1;
            *seg.offset *= 2;

            // remove the leading zero and shift left
            now &= !mask;
            now <<= 1;

            times.push(seg);
            *seg.offset += 1;
            max -= 1;
            if max == 0 {
                break;
            }
            if now & mask > 0 {
                // if the MSB is 1, duplicate the segment
                times.push(seg);
                *seg.offset += 1;
                max -= 1;
                if max == 0 {
                    break;
                }
            }
        }
        if self.limit.is_none() {
            // Should be all zeroes at this point
            debug_assert_eq!(now & !mask, 0)
        }
        times
    }

    /// Set a limit
    pub fn limit(&self, limit: u32) -> Self {
        Self {
            time: self.time,
            limit: Some(limit),
        }
    }

    /// Modify the region data associated with two different TelescopingTimes
    /// of different lengths, so that both data vectors are referring to
    /// the same regions.
    ///
    /// In general, when one TelescopingTimes sequence is longer than another,
    /// the longer sequence will have larger TimeSegments than the shorter one.
    /// To rectify them, the shorter sequence needs to merge some of its earlier
    /// data until it has a segment large enough to match the larger segment
    /// of the other sequence. This continues until all segments of the smaller
    /// sequence are exhausted. Then, the longer sequence is truncated to match
    /// the shorter one.
    pub fn rectify<T: AddAssign>(a: (&Self, &mut Vec<T>), b: (&Self, &mut Vec<T>)) {
        let (left, right) = if a.0.time > b.0.time { (b, a) } else { (a, b) };
        let (lt, ld) = left;
        let (rt, rd) = right;
        let mut lt: Vec<_> = lt.segments().iter().map(TimeSegment::num_quanta).collect();
        let rt: Vec<_> = rt.segments().iter().map(TimeSegment::num_quanta).collect();
        assert_eq!(lt.len(), ld.len());
        assert_eq!(rt.len(), rd.len());
        let mut i = 0;
        while i < lt.len() - 1 {
            while lt[i] < rt[i] && i < lt.len() - 1 {
                lt[i] += lt.remove(i + 1);
                let d = ld.remove(i + 1);
                ld[i] += d;
            }
            i += 1;
        }
        rd.truncate(ld.len());
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn segment_length() {
        let s = TimeSegment::new(31, 0);
        assert_eq!(s.num_quanta(), 2u64.pow(31));
    }

    fn lengths(t: TimeQuantum) -> Vec<u32> {
        TelescopingTimes::new(t)
            .segments()
            .into_iter()
            .map(|i| i.num_quanta() as u32)
            .collect()
    }

    #[test]
    fn test_telescoping_times_limit() {
        let tt = TelescopingTimes::new(64.into());
        assert_eq!(tt.segments().len(), 7);
        assert_eq!(tt.limit(6).segments().len(), 6);
        assert_eq!(tt.limit(4).segments().len(), 4);
        assert_eq!(
            tt.segments().into_iter().take(6).collect::<Vec<_>>(),
            tt.limit(6).segments()
        );
    }

    #[test]
    #[rustfmt::skip]
    fn test_telescoping_times_first_16() {
        let ts = TimeQuantum::from;

                                                             // n+1
        assert_eq!(lengths(ts(0)),  Vec::<u32>::new());      // 0001
        assert_eq!(lengths(ts(1)),  vec![1]);                // 0010
        assert_eq!(lengths(ts(2)),  vec![1, 1]);             // 0011
        assert_eq!(lengths(ts(3)),  vec![2, 1]);             // 0100
        assert_eq!(lengths(ts(4)),  vec![2, 1, 1]);          // 0101
        assert_eq!(lengths(ts(5)),  vec![2, 2, 1]);          // 0110
        assert_eq!(lengths(ts(6)),  vec![2, 2, 1, 1]);       // 0111
        assert_eq!(lengths(ts(7)),  vec![4, 2, 1]);          // 1000
        assert_eq!(lengths(ts(8)),  vec![4, 2, 1, 1]);       // 1001
        assert_eq!(lengths(ts(9)),  vec![4, 2, 2, 1]);       // 1010
        assert_eq!(lengths(ts(10)), vec![4, 2, 2, 1, 1]);    // 1011
        assert_eq!(lengths(ts(11)), vec![4, 4, 2, 1]);       // 1100
        assert_eq!(lengths(ts(12)), vec![4, 4, 2, 1, 1]);    // 1101
        assert_eq!(lengths(ts(13)), vec![4, 4, 2, 2, 1]);    // 1110
        assert_eq!(lengths(ts(14)), vec![4, 4, 2, 2, 1, 1]); // 1111
        assert_eq!(lengths(ts(15)), vec![8, 4, 2, 1]);      // 10000
    }

    /// Test that data generated by two different telescoping time sets can be
    /// rectified.
    ///
    /// The data used in this test are simple vecs of integers, but in the real
    /// world, the data would be the region data (which has an AddAssign impl).
    #[test]
    fn test_rectify_telescoping_times() {
        {
            let a = TelescopingTimes::new(5.into());
            let b = TelescopingTimes::new(8.into());

            // the actual integers used here don't matter,
            // they're just picked so that sums look distinct
            let mut da = vec![16, 8, 4];
            let mut db = vec![32, 16, 8, 4];
            TelescopingTimes::rectify((&a, &mut da), (&b, &mut db));
            assert_eq!(da, vec![16 + 8, 4]);
            assert_eq!(db, vec![32, 16]);
        }
        {
            let a = TelescopingTimes::new(14.into());
            let b = TelescopingTimes::new(16.into());
            let mut da = vec![128, 64, 32, 16, 8, 4];
            let mut db = vec![32, 16, 8, 4, 1];
            TelescopingTimes::rectify((&a, &mut da), (&b, &mut db));
            assert_eq!(da, vec![128 + 64, 32 + 16, 8 + 4]);
            assert_eq!(db, vec![32, 16, 8]);
        }
    }

    proptest::proptest! {
        #[test]
        fn telescoping_times_cover_total_time_span(now in 0u32..u32::MAX) {
            let topo = Topology::unit_zero();
            let ts = TelescopingTimes::new(now.into()).segments();
            let total = ts.iter().fold(0u64, |len, t| {
                assert_eq!(t.quantum_bounds(&topo).0.inner(), len as u32, "t = {:?}, len = {}", t, len);
                len + t.num_quanta()
            });
            assert_eq!(total, now as u64);
        }

        #[test]
        fn telescoping_times_end_with_1(now: u32) {
            if let Some(last) = TelescopingTimes::new(now.into()).segments().pop() {
                assert_eq!(last.power, 0);
            }
        }

        #[test]
        fn telescoping_times_are_fractal(now: u32) {
            let a = lengths(now.into());
            let b = lengths((now - a[0]).into());
            assert_eq!(b.as_slice(), &a[1..]);
        }

        #[test]
        fn rectification_doesnt_panic(a: u32, b: u32) {
            let (a, b) = if a < b { (a, b)} else {(b, a)};
            let a = TelescopingTimes::new(a.into());
            let b = TelescopingTimes::new(b.into());
            let mut da = vec![1; a.segments().len()];
            let mut db = vec![1; b.segments().len()];
            TelescopingTimes::rectify((&a, &mut da), (&b, &mut db));
            assert_eq!(da.len(), db.len());
        }
    }
}
