pub enum RateLimitError {
    MaxCountExceeded,
    FrequencyExceeded,
}

pub struct RateLimit {
    pub max: Option<u64>,
    pub count: u64,

    pub last: std::time::Instant,
    pub period: Option<std::time::Duration>,
}

impl RateLimit {
    pub fn unlimited() -> Self {
        Self {
            max: None,
            count: 0,
            last: std::time::Instant::now(),
            period: None,
        }
    }

    pub fn new(max: u64, freq_per_second: f32) -> Self {
        let p = std::time::Duration::from_secs_f32(1.0 / freq_per_second);
        Self {
            max: Some(max),
            count: 0,
            last: std::time::Instant::now() - p,
            period: Some(p),
        }
    }

    pub fn new_max_count(max: u64) -> Self {
        Self {
            max: Some(max),
            count: 0,
            last: std::time::Instant::now(),
            period: None,
        }
    }

    pub fn new_frequency(freq_per_second: f32) -> Self {
        let p = std::time::Duration::from_secs_f32(1.0 / freq_per_second);
        Self {
            max: None,
            count: 0,
            last: std::time::Instant::now() - p,
            period: Some(p),
        }
    }
}

impl RateLimit {
    pub fn check(&mut self) -> Result<(), RateLimitError> {
        if let Some(max) = self.max {
            if self.count >= max {
                return Err(RateLimitError::MaxCountExceeded);
            }
        }

        if let Some(period) = self.period {
            let now = std::time::Instant::now();
            if now - self.last < period {
                return Err(RateLimitError::FrequencyExceeded);
            }
            self.last = now;
        }
        Ok(())
    }

    pub fn increment(&mut self) {
        self.count += 1;
        self.last = std::time::Instant::now();
    }

    pub async fn wait_and_increment_async(&mut self) {
        if let Some(period) = self.period {
            let now = std::time::Instant::now();
            let wait = period.checked_sub(now - self.last).unwrap_or_default();
            self.count += 1;
            self.last = now + wait;
            if wait > std::time::Duration::from_secs(0) {
                tokio::time::sleep(period - (now - self.last)).await;
            }
        }
    }

    pub fn wait(&mut self) {
        if let Some(period) = self.period {
            let now = std::time::Instant::now();
            if now - self.last < period {
                std::thread::sleep(period - (now - self.last));
            }
        }
    }

    pub fn wait_and_increment(&mut self) {
        self.wait();
        self.increment();
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.last = std::time::Instant::now() - self.period.unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_frequency_rate_limit() {
        let mut rl = super::RateLimit::new_frequency(1.0);
        for i in 0..10 {
            let now = std::time::Instant::now();
            rl.wait_and_increment();
            assert!(
                i == 0
                    || std::time::Instant::now() - now
                        > std::time::Duration::from_secs_f32(1.0)
            );
        }
    }
    #[tokio::test]
    async fn test_frequency_rate_limit_async() {
        let mut rl = super::RateLimit::new_frequency(1.0);
        for i in 0..10 {
            let now = std::time::Instant::now();
            rl.wait_and_increment_async().await;
            println!("{:?}", std::time::Instant::now() - now);
            assert!(
                i == 0
                    || std::time::Instant::now() - now
                        > std::time::Duration::from_secs_f32(1.0)
            );
        }
    }
}
