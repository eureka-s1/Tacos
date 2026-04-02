//! Kernel Threads

mod imp;
pub mod manager;
pub mod scheduler;
pub mod switch;

pub use self::imp::*;
pub use self::manager::Manager;
pub(self) use self::scheduler::{Schedule, Scheduler};

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::Ordering::SeqCst;

use crate::sbi;
use crate::sync::{Intr, Lazy, Mutex as SyncMutex};

#[derive(Clone)]
struct SleepEntry {
    wake_tick: i64,
    thread: Arc<Thread>,
}

static SLEEP_LIST: Lazy<SyncMutex<Vec<SleepEntry>, Intr>> =
    Lazy::new(|| SyncMutex::new(Vec::new()));

#[inline]
fn clamp_priority(priority: u32) -> u32 {
    priority.clamp(PRI_MIN, PRI_MAX)
}

/// Create a new thread
pub fn spawn<F>(name: &'static str, f: F) -> Arc<Thread>
where
    F: FnOnce() + Send + 'static,
{
    Builder::new(f).name(name).spawn()
}

/// Get the current running thread
pub fn current() -> Arc<Thread> {
    Manager::get().current.lock().clone()
}

/// Yield the control to another thread (if there's another one ready to run).
pub fn schedule() {
    Manager::get().schedule()
}

/// Gracefully shut down the current thread, and schedule another one.
pub fn exit() -> ! {
    {
        let current = Manager::get().current.lock();

        #[cfg(feature = "debug")]
        kprintln!("Exit: {:?}", *current);

        current.set_status(Status::Dying);
    }

    schedule();

    unreachable!("An exited thread shouldn't be scheduled again");
}

/// Mark the current thread as [`Blocked`](Status::Blocked) and
/// yield the control to another thread
pub fn block() {
    let current = current();
    current.set_status(Status::Blocked);

    #[cfg(feature = "debug")]
    kprintln!("[THREAD] Block {:?}", current);

    schedule();
}

/// Wake up a previously blocked thread, mark it as [`Ready`](Status::Ready),
/// and register it into the scheduler.
pub fn wake_up(thread: Arc<Thread>) {
    assert_eq!(thread.status(), Status::Blocked);
    thread.set_status(Status::Ready);

    #[cfg(feature = "debug")]
    kprintln!("[THREAD] Wake up {:?}", thread);

    Manager::get().scheduler.lock().register(thread);
}

/// (Lab1) Sets the current thread's priority to a given value
pub fn set_priority(priority: u32) {
    let priority = clamp_priority(priority);
    let current = current();
    let old = sbi::interrupt::set(false);
    let previous = current.priority.swap(priority, SeqCst);

    if priority < previous {
        schedule();
    }

    sbi::interrupt::set(old);
}

/// (Lab1) Returns the current thread's effective priority.
pub fn get_priority() -> u32 {
    current().priority.load(SeqCst)
}

/// (Lab1) Make the current thread sleep for the given ticks.
pub fn sleep(ticks: i64) {
    use crate::sbi::timer::timer_ticks;

    if ticks <= 0 {
        return;
    }

    let wake_tick = timer_ticks() + ticks;
    let old = sbi::interrupt::set(false);

    {
        let mut sleepers = SLEEP_LIST.lock();
        let current = current();
        let idx = sleepers.partition_point(|entry| entry.wake_tick <= wake_tick);
        sleepers.insert(
            idx,
            SleepEntry {
                wake_tick,
                thread: current,
            },
        );
    }

    block();
    sbi::interrupt::set(old);
}

pub(crate) fn wake_sleeping_threads() {
    let now = crate::sbi::timer::timer_ticks();
    let due = {
        let mut sleepers = SLEEP_LIST.lock();
        let count = sleepers.partition_point(|entry| entry.wake_tick <= now);
        sleepers
            .drain(0..count)
            .map(|entry| entry.thread)
            .collect::<Vec<_>>()
    };

    for thread in due {
        wake_up(thread);
    }
}
