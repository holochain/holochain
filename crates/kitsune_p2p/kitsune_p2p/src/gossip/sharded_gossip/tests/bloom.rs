use std::collections::BTreeMap;
use std::slice::SliceIndex;

use super::common::*;
use super::*;
use crate::gossip::sharded_gossip::bloom::Batch;
use crate::HostStub;

#[tokio::test(flavor = "multi_thread")]
async fn bloom_windows() {
    let expected_time = time_range(Duration::from_secs(20), Duration::from_secs(2));
    let search_window = time_range(
        std::time::UNIX_EPOCH.elapsed().unwrap(),
        Duration::from_secs(0),
    );

    let r = make_node(1, expected_time.clone())
        .await
        .generate_ops_blooms_for_time_window(&Arc::new(DhtArcSet::Full), search_window.clone())
        .await
        .unwrap();

    match r {
        Batch::Complete(v) => {
            assert_eq!(v.len(), 1);
            let r = v.first().unwrap();
            let TimedBloomFilter { bloom, time } = r;
            assert!(bloom.is_some());
            assert_eq!(*time, search_window);
        }
        _ => unreachable!(),
    }

    let r = make_empty_node()
        .await
        .generate_ops_blooms_for_time_window(&Arc::new(DhtArcSet::Full), search_window.clone())
        .await
        .unwrap();

    match r {
        Batch::Complete(v) => {
            assert_eq!(v.len(), 1);
            let r = v.first().unwrap();
            let TimedBloomFilter { bloom, time } = r;
            assert!(bloom.is_none());
            assert_eq!(*time, search_window);
        }
        _ => unreachable!(),
    }

    let r = make_node(
        ShardedGossipLocal::UPPER_HASHES_BOUND - 1,
        expected_time.clone(),
    )
    .await
    .generate_ops_blooms_for_time_window(&Arc::new(DhtArcSet::Full), search_window.clone())
    .await
    .unwrap();

    match r {
        Batch::Complete(v) => {
            assert_eq!(v.len(), 1);
            let r = v.first().unwrap();
            let TimedBloomFilter { bloom, time } = r;
            assert!(bloom.is_some());
            assert_eq!(*time, search_window);
        }
        _ => unreachable!(),
    }

    let r = make_node(
        ShardedGossipLocal::UPPER_HASHES_BOUND,
        expected_time.clone(),
    )
    .await
    .generate_ops_blooms_for_time_window(&Arc::new(DhtArcSet::Full), search_window.clone())
    .await
    .unwrap();

    match r {
        Batch::Complete(v) => {
            assert_eq!(v.len(), 2);
            let r = v.get(0).unwrap();
            let TimedBloomFilter { bloom, time } = r;
            assert!(bloom.is_some());
            assert_eq!(
                *time,
                search_window.start
                    ..get_time_bounds(
                        ShardedGossipLocal::UPPER_HASHES_BOUND as u32,
                        expected_time.clone(),
                        ..ShardedGossipLocal::UPPER_HASHES_BOUND
                    )
                    .end
            );
            let r = v.get(1).unwrap();
            let TimedBloomFilter { bloom, time } = r;
            assert!(bloom.is_some());
            assert_eq!(
                *time,
                get_time_bounds(
                    ShardedGossipLocal::UPPER_HASHES_BOUND as u32,
                    expected_time.clone(),
                    (ShardedGossipLocal::UPPER_HASHES_BOUND - 1)
                        ..ShardedGossipLocal::UPPER_HASHES_BOUND
                )
                .start..search_window.end
            );
        }
        _ => unreachable!(),
    }

    let r = make_node(
        ShardedGossipLocal::UPPER_HASHES_BOUND * ShardedGossipLocal::UPPER_BLOOM_BOUND,
        expected_time.clone(),
    )
    .await
    .generate_ops_blooms_for_time_window(&Arc::new(DhtArcSet::Full), search_window.clone())
    .await
    .unwrap();

    let last_cursor;
    match r {
        Batch::Partial { data: v, cursor } => {
            assert_eq!(v.len(), ShardedGossipLocal::UPPER_BLOOM_BOUND);
            let total =
                ShardedGossipLocal::UPPER_HASHES_BOUND * ShardedGossipLocal::UPPER_BLOOM_BOUND;
            // We use the same timestamp from the end of the last bloom as the beginning of the
            // next bloom incase there are multiple hashes with the same timestamp.
            // So we expect the cursor to land on the time for the last hash - 1 * UPPER_BLOOM_BOUND.
            let end_of_blooms_time = (get_time_bounds(
                total as u32,
                expected_time.clone(),
                ..=(total - ShardedGossipLocal::UPPER_BLOOM_BOUND),
            )
            // Take off the micro second that is added to make the bounds exclusive
            // because the cursor is the actual time of the last hash seen.
            .end - Duration::from_micros(1))
            .unwrap();
            assert_eq!(cursor, end_of_blooms_time);
            last_cursor = cursor;

            let mut expected_window = search_window.clone();
            for (i, TimedBloomFilter { bloom, time }) in v.into_iter().enumerate() {
                assert!(bloom.is_some());

                expected_window.end = get_time_bounds(
                    total as u32,
                    expected_time.clone(),
                    ..ShardedGossipLocal::UPPER_HASHES_BOUND
                        + i * ShardedGossipLocal::UPPER_HASHES_BOUND
                        - i,
                )
                .end;
                dbg!(i);
                eprintln!("{:?} -> {:?}", time, expected_window);
                assert_eq!(time, expected_window);

                // The next bloom starts at the actual last blooms last time (not the exclusive 1us)
                expected_window.start = (expected_window.end - Duration::from_micros(1)).unwrap();
            }
        }
        _ => unreachable!(),
    }

    let r = make_node(
        ShardedGossipLocal::UPPER_HASHES_BOUND * ShardedGossipLocal::UPPER_BLOOM_BOUND,
        expected_time.clone(),
    )
    .await
    .generate_ops_blooms_for_time_window(&Arc::new(DhtArcSet::Full), last_cursor..search_window.end)
    .await
    .unwrap();

    match r {
        Batch::Complete(v) => {
            assert_eq!(v.len(), 1);
            let r = v.get(0).unwrap();
            let TimedBloomFilter { bloom, time } = r;
            assert!(bloom.is_some());
            assert_eq!(*time, last_cursor..search_window.end);
        }
        _ => unreachable!(),
    }
}

async fn make_node(num: usize, window: TimeWindow) -> ShardedGossipLocal {
    make_node_inner(Some((num, window))).await
}

async fn make_empty_node() -> ShardedGossipLocal {
    make_node_inner(None).await
}

async fn make_node_inner(data: Option<(usize, TimeWindow)>) -> ShardedGossipLocal {
    let mut evt_handler = MockKitsuneP2pEventHandler::new();
    let data = data.map(|(n, time)| {
        let len = time.end - time.start;
        let step = dbg!(len.unwrap().to_std().unwrap()) / dbg!(n as u32);
        dbg!(step);
        (0..n)
            .map(|_| Arc::new(KitsuneOpHash(vec![0; 36])))
            .enumerate()
            .map(|(i, data)| ((time.start + step * i as u32).unwrap().as_micros(), data))
            .collect::<BTreeMap<_, _>>()
    });
    evt_handler
        .expect_handle_query_op_hashes()
        .returning(move |input| {
            let data = data.clone();
            let data = data.and_then(|data| {
                let start = data
                    .range(input.window.start.as_micros()..input.window.end.as_micros())
                    .next()?
                    .0;
                let end = data
                    .range(input.window.start.as_micros()..input.window.end.as_micros())
                    .take(input.max_ops)
                    .last()?
                    .0;
                eprintln!(
                    "{} -> {}",
                    input.window.start.as_micros(),
                    input.window.end.as_micros()
                );
                eprintln!("{} -> {}", start, end);
                Some((
                    data.range(input.window.start.as_micros()..input.window.end.as_micros())
                        .map(|(_, d)| d.clone())
                        .take(input.max_ops)
                        .collect(),
                    Timestamp::from_micros(*start)..=Timestamp::from_micros(*end),
                ))
            });
            Ok(async move { Ok(data) }.boxed().into())
        });
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let host = HostStub::new();

    ShardedGossipLocal::test(
        GossipType::Historical,
        evt_sender,
        host,
        ShardedGossipLocalState::default(),
    )
}

fn get_time_bounds(
    n: u32,
    window: TimeWindow,
    records: impl SliceIndex<[Timestamp], Output = [Timestamp]>,
) -> TimeWindow {
    let len = window.end - window.start;
    let step = len.unwrap().to_std().unwrap() / n;
    let times = (0..n)
        .map(|i| (window.start + step * i as u32).unwrap())
        .collect::<Vec<_>>();

    let mut iter = times[records].iter();
    let start = iter.next().unwrap();
    let mut end = *iter.last().unwrap_or(start);
    end = (end + Duration::from_micros(1)).unwrap();
    Timestamp::from_micros(start.as_micros())..Timestamp::from_micros(end.as_micros())
}
