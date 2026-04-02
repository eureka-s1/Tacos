use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use core::cell::RefCell;

use crate::sbi;
use crate::sync::{Intr, Lazy, Mutex};
use crate::sync::{Lock, Semaphore};
use crate::thread::{self, Thread};

/// Sleep lock. Uses [`Semaphore`] under the hood.
#[derive(Clone)]
pub struct Sleep {
    inner: Semaphore,
    holder: RefCell<Option<Arc<Thread>>>,
}

static LOCK_HOLDERS: Lazy<Mutex<BTreeMap<usize, Arc<Thread>>, Intr>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

impl Default for Sleep {
    fn default() -> Self {
        Self {
            inner: Semaphore::new(1),
            holder: Default::default(),
        }
    }
}

impl Lock for Sleep {
    fn acquire(&self) {
        let old = sbi::interrupt::set(false);
        let lock_id = self.lock_id();

        if self.holder.borrow().is_some() {
            *thread::current().waiting_lock.lock() = Some(lock_id);
            thread::donate_for_lock(lock_id);
        }

        self.inner.down();
        let current = thread::current();
        current.waiting_lock.lock().take();
        self.holder.borrow_mut().replace(current.clone());
        LOCK_HOLDERS.lock().insert(lock_id, current);
        sbi::interrupt::set(old);
    }

    fn release(&self) {
        let old = sbi::interrupt::set(false);
        assert!(Arc::ptr_eq(
            self.holder.borrow().as_ref().unwrap(),
            &thread::current()
        ));

        let current = thread::current();
        let lock_id = self.lock_id();
        let (before, after) = thread::remove_lock_donations(&current, lock_id);
        self.holder.borrow_mut().take().unwrap();
        LOCK_HOLDERS.lock().remove(&lock_id);
        self.inner.up();

        if old && after < before {
            thread::schedule();
        }
        sbi::interrupt::set(old);
    }
}

unsafe impl Sync for Sleep {}

impl Sleep {
    #[inline]
    pub(crate) fn lock_id(&self) -> usize {
        self as *const _ as usize
    }
}

pub(crate) fn holder_by_id(lock_id: usize) -> Option<Arc<Thread>> {
    LOCK_HOLDERS.lock().get(&lock_id).cloned()
}
