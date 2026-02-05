#[cfg(feature = "nfs")]
mod dotlock;
mod flock;

#[cfg(feature = "nfs")]
pub use dotlock::{create_lock, lock_mtime, remove_lock};
pub use flock::FileLock;
