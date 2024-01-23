pub mod runtime;

#[cfg(test)]
mod tests {
    use futures::executor::{LocalPool, ThreadPool};
    use tokio::time::sleep;

    #[test]
    fn test_no_async_execution_context() {
        let pool = ThreadPool::new().unwrap();
        pool.spawn_ok(async {
            sleep(std::time::Duration::from_millis(10)).await;
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_with_tokio_context() {
        let mut pool = LocalPool::new();
        pool.run_until(async {
            sleep(std::time::Duration::from_millis(10)).await;
            println!("async task runs under local pool");
        });
    }
}
