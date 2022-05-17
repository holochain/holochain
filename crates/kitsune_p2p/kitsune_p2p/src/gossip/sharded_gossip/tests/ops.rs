use std::collections::VecDeque;

use kitsune_p2p_types::dht::{
    prelude::Segment,
    region::{Region, RegionCoords, RegionData},
};

fn region_queue_iteration(
    queue: &mut VecDeque<Region>,
    batch_size: u32,
) -> (Vec<Region>, Option<Region>, bool) {
    let mut size = 0;
    let mut to_fetch = vec![];
    let mut to_split = None;
    let mut finished = true;
    while let Some(region) = queue.pop_front() {
        if region.data.size > batch_size {
            to_split = Some(region);
            break;
        }
        size += region.data.size;
        if size > batch_size {
            queue.push_front(region);
            finished = false;
            break;
        } else {
            to_fetch.push(region);
        }
    }
    (to_fetch, to_split, finished)
}

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

fn run(queue: &mut VecDeque<Region>, batch_size: u32) -> (Vec<u32>, Option<u32>, bool) {
    let (fetch, split, fin) = region_queue_iteration(queue, batch_size);
    (
        fetch.into_iter().map(|r| r.data.size).collect(),
        split.map(|r| r.data.size),
        fin,
    )
}

#[test]
fn test_region_queue() {
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
    assert_eq!(r, (vec![1000, 2000], None, false));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), initial_len - 4);
    assert_eq!(r, (vec![3000], Some(5000), false));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), initial_len - 5);
    assert_eq!(r, (vec![], Some(8000), false));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), initial_len - 7);
    assert_eq!(r, (vec![1000, 2000], None, false));

    let r = run(&mut queue, BATCH_SIZE);
    assert_eq!(queue.len(), 0);
    assert_eq!(r, (vec![3000], None, true));
}
