use core::sync::atomic::{AtomicI32, Ordering};

use crate::Errno;

static ERRNO_CODE: AtomicI32 = AtomicI32::new(0);
const DESCRIPTION: &str = "unsupported platform";

pub fn with_description<F, T>(err: Errno, callback: F) -> T
where
    F: FnOnce(Result<&str, Errno>) -> T,
{
    let result = if err.0 == 0 {
        Ok("success")
    } else {
        Ok(DESCRIPTION)
    };
    callback(result)
}

pub const STRERROR_NAME: &str = "unsupported";

pub fn errno() -> Errno {
    Errno(ERRNO_CODE.load(Ordering::Relaxed))
}

pub fn set_errno(err: Errno) {
    ERRNO_CODE.store(err.0, Ordering::Relaxed);
}
