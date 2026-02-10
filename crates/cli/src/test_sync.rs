use std::sync::{Mutex, MutexGuard};

static CWD_LOCK: Mutex<()> = Mutex::new(());

pub fn lock_cwd() -> MutexGuard<'static, ()> {
	CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
