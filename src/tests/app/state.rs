use crate::app::select_ready_shard;

#[test]
fn selects_a_healthy_shard() {
    let available = [false, false, true, false];
    assert_eq!(
        select_ready_shard(0, available.len(), |i| available[i]),
        Some(2)
    );
    assert_eq!(
        select_ready_shard(3, available.len(), |i| available[i]),
        Some(2)
    );
}

#[test]
fn distributes_round_robin() {
    let available = [true, false, true, false];
    let indices: Vec<usize> = (0..4)
        .map(|start| select_ready_shard(start, available.len(), |i| available[i]).unwrap())
        .collect();
    assert_eq!(indices, vec![0, 2, 2, 0]);
}

#[test]
fn waits_shards_ready() {
    assert_eq!(select_ready_shard(0, 4, |_| false), None);
    assert_eq!(select_ready_shard(0, 0, |_| true), None);
}
