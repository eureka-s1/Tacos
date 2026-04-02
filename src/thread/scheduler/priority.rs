use alloc::collections::VecDeque;
use alloc::sync::Arc;

use crate::thread::{Schedule, Thread, PRI_MAX, PRI_MIN};

/// Priority scheduler: higher-priority thread runs first, same-priority threads run in RR.
pub struct Priority([VecDeque<Arc<Thread>>; (PRI_MAX as usize) + 1]);

impl Default for Priority {
    fn default() -> Self {
        Self(core::array::from_fn(|_| VecDeque::new()))
    }
}

impl Schedule for Priority {
    fn register(&mut self, thread: Arc<Thread>) {
        let priority = thread.priority.load(core::sync::atomic::Ordering::SeqCst) as usize;
        self.0[priority].push_front(thread);
    }

    fn schedule(&mut self) -> Option<Arc<Thread>> {
        for priority in (PRI_MIN..=PRI_MAX).rev() {
            if let Some(next) = self.0[priority as usize].pop_back() {
                return Some(next);
            }
        }
        None
    }
}
