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

fn effective_priority(thread: &Arc<Thread>) -> u32 {
    let base = thread.base_priority.load(SeqCst);
    let donated = thread
        .donations
        .lock()
        .iter()
        .map(|d| d.priority)
        .max()
        .unwrap_or(PRI_MIN);
    base.max(donated)
}

fn refresh_effective_priority(thread: &Arc<Thread>) -> u32 {
    let next = effective_priority(thread);
    thread.priority.swap(next, SeqCst)
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
    let base = clamp_priority(priority);
    let current = current();
    let old = sbi::interrupt::set(false);
    current.base_priority.store(base, SeqCst);
    let previous = refresh_effective_priority(&current);
    let current_effective = current.priority.load(SeqCst);

    if current_effective < previous {
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

pub(crate) fn donate_for_lock(lock_id: usize) {
    let donor = current();
    let donor_tid = donor.id();
    let mut donated_priority = donor.priority.load(SeqCst);
    let mut waiting_lock = Some(lock_id);

    while let Some(lock_id) = waiting_lock {
        let Some(holder) = crate::sync::sleep::holder_by_id(lock_id) else {
            break;
        };

        if holder.id() == donor_tid {
            break;
        }

        {
            let mut donations = holder.donations.lock();
            if let Some(entry) = donations
                .iter_mut()
                .find(|entry| entry.donor_tid == donor_tid && entry.lock_id == lock_id)
            {
                entry.priority = donated_priority;
            } else {
                donations.push(Donation {
                    donor_tid,
                    lock_id,
                    priority: donated_priority,
                });
            }
        }

        refresh_effective_priority(&holder);
        donated_priority = holder.priority.load(SeqCst);
        waiting_lock = *holder.waiting_lock.lock();
    }
}

pub(crate) fn remove_lock_donations(thread: &Arc<Thread>, lock_id: usize) -> (u32, u32) {
    let before = thread.priority.load(SeqCst);
    thread
        .donations
        .lock()
        .retain(|entry| entry.lock_id != lock_id);
    let _old = refresh_effective_priority(thread);
    let after = thread.priority.load(SeqCst);
    (before, after)
}
