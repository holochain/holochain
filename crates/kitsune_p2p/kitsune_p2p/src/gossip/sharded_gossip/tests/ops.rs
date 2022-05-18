use std::collections::VecDeque;

use kitsune_p2p_types::dht::{
    prelude::Segment,
    region::{Region, RegionCoords, RegionData},
};

use crate::gossip::sharded_gossip::ops::get_region_queue_batch;

fn fake_region(count: u32, size: u32) -> Region {
    Region {
        coords: RegionCoords {
            space: Segment::new(0, 0),
            time: Segment::new(0, 0),
        },
        data: RegionData {
            hash: [0; 32].into(),
            count,
            size,
        },
    }
}

#[test]
fn test_region_queue() {
    fn run(queue: &mut VecDeque<Region>, batch_size: u32) -> Vec<u32> {
        get_region_queue_batch(queue, batch_size)
            .into_iter()
            .map(|r| r.data.size)
            .collect()
    }

    const BATCH_SIZE: u32 = 4000;
    let mut queue: VecDeque<_> = vec![
        fake_region(1, 1000),
        fake_region(2, 2000),
        fake_region(3, 3000),
        fake_region(5, 5000),
        fake_region(8, 8000),
        fake_region(101, 1000),
        fake_region(102, 2000),
        fake_region(103, 3000),
    ]
    .into();
    let initial_len = queue.len();

    assert_eq!(queue.len(), initial_len);

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), initial_len - 2);
    assert_eq!(r, (vec![1000, 2000]));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), initial_len - 4);
    assert_eq!(r, (vec![3000]));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), initial_len - 5);
    assert_eq!(r, (vec![5000]));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), initial_len - 6);
    assert_eq!(r, (vec![8000]));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), initial_len - 7);
    assert_eq!(r, (vec![1000, 2000]));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), 0);
    assert_eq!(r, (vec![3000]));
}
