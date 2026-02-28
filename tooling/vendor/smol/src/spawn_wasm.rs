use std::future::Future;
use std::sync::LazyLock;

use async_executor::{Executor, Task};

static EXECUTOR: LazyLock<Executor<'static>> = LazyLock::new(Executor::new);

pub fn spawn<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> Task<T> {
    EXECUTOR.spawn(future)
}
