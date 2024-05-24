#[cfg(loom)]
mod tests {
    use loom::sync::atomic::AtomicUsize;

    use std::sync::atomic::Ordering::SeqCst;
    use std::sync::Arc;

    #[test]
    fn test_concurrent_logic() {
        loom::model(|| {
            let v1 = Arc::new(AtomicUsize::new(0));
            let v2 = v1.clone();

            assert_eq!(0, v2.load(SeqCst));
        });
    }
}
