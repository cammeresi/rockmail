#[cfg(feature = "nfs")]
mod dotlock;
mod flock;

/// Lockfiles should not be larger than this (procmail's MAX_locksize).
pub const MAX_LOCK_SIZE: u64 = 16;

#[cfg(feature = "nfs")]
pub use dotlock::{create_lock, lock_mtime, remove_lock};
pub use flock::FileLock;
