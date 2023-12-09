use crate::OptionDebugger;
use async_std::task::JoinHandle;

/// This struct will wrapper a JoinHandler and auto cancel the task when it drop.
/// It is useful when we spawn some background task, which depend on current task
pub struct AutoCancelTask<T: 'static> {
    handle: Option<JoinHandle<T>>,
}

impl<T: 'static> From<JoinHandle<T>> for AutoCancelTask<T> {
    fn from(value: JoinHandle<T>) -> Self {
        Self { handle: Some(value) }
    }
}

impl<T: 'static> Drop for AutoCancelTask<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            async_std::task::spawn_local(async move {
                handle.cancel().await.log_option("Should cancel task");
            });
        }
    }
}
