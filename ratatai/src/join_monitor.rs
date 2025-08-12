use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use tokio::task::JoinHandle;
use tracing::{error, warn};

pub struct JoinHandleMonitor<T> {
    handle: Pin<Box<JoinHandle<T>>>,
    waker: Waker,
}

impl<T> JoinHandleMonitor<T> {
    pub fn new(handle: JoinHandle<T>) -> Self {
        let waker = dummy_waker();
        Self {
            handle: Box::pin(handle),
            waker,
        }
    }

    pub fn is_finished(&mut self) -> Option<Result<T, tokio::task::JoinError>> {
        let mut cx = Context::from_waker(&self.waker);
        match self.handle.as_mut().poll(&mut cx) {
            Poll::Ready(res) => Some(res),
            Poll::Pending => None,
        }
    }
}

fn dummy_waker() -> Waker {
    struct Dummy;
    impl Wake for Dummy {
        fn wake(self: Arc<Self>) {}
    }
    std::task::Waker::from(Arc::new(Dummy))
}

pub fn check_monitor(monitor: &mut JoinHandleMonitor<()>) -> bool {
    if let Some(result) = monitor.is_finished() {
        match result {
            Ok(_) => warn!("⚠️ Chat task stopped."),
            Err(e) => error!("💥 Chat task panicked : {e}"),
        }
        return true;
    }
    false
}
