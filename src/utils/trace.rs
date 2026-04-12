use crate::utils::cache::trace_cache::TracePoint;
use std::time::Instant;

pub struct TraceContext {
    start: Instant,
    last: Instant,
    pub points: Vec<TracePoint>,
}

impl TraceContext {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            points: Vec::new(),
        }
    }

    pub fn point(&mut self, name: impl Into<String>) {
        let now = Instant::now();
        self.points.push(TracePoint {
            name: name.into(),
            duration: now.duration_since(self.last),
        });
        self.last = now;
    }

    pub fn start_time(&self) -> Instant {
        self.start
    }
}
