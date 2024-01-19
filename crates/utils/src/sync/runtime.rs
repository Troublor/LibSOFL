use std::future::Future;

pub struct AsyncRuntime {
    runtime: Option<tokio::runtime::Runtime>,
}

impl AsyncRuntime {
    pub fn new() -> Self {
        let runtime = match tokio::runtime::Handle::try_current() {
            Ok(_) => None,
            Err(_) => {
                let runtime = tokio::runtime::Runtime::new()
                    .expect("build tokio runtime");
                Some(runtime)
            }
        };
        Self { runtime }
    }
}

impl AsyncRuntime {
    pub fn block_on<F: Future>(&self, f: F) -> F::Output {
        match self.runtime {
            Some(ref rt) => rt.block_on(f),
            None => futures::executor::block_on(f),
        }
    }

    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        match self.runtime {
            Some(ref rt) => rt.spawn_blocking(f),
            None => tokio::task::spawn_blocking(f),
        }
    }
}

impl Clone for AsyncRuntime {
    fn clone(&self) -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::sleep;

    #[test]
    fn test_no_tokio_runtime() {
        let rt = super::AsyncRuntime::new();
        rt.block_on(async {
            sleep(Duration::from_millis(10)).await;
            println!("async task runs without tokio runtime");
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_under_tokio_runtime() {
        let rt = super::AsyncRuntime::new();
        rt.block_on(async {
            sleep(Duration::from_millis(10)).await;
            println!("async task runs under tokio runtime");
        });
    }
}
