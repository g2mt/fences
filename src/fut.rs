use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use parking_lot::Mutex;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use crate::desktop_cover::WM_USER_WAKE_FUTURE;

pub struct PromptState<T> {
    pub result: Option<T>,
    pub waker: Option<Waker>,
    pub completed: bool,
}

pub struct PromptFuture<T> {
    pub state: Arc<Mutex<PromptState<T>>>,
}

impl<T> Future for PromptFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();
        if state.completed {
            Poll::Ready(state.result.take().unwrap())
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct WindowWaker {
    pub hwnd: HWND,
}

impl std::task::Wake for WindowWaker {
    fn wake(self: Arc<Self>) {
        unsafe {
            let _ = PostMessageW(self.hwnd, WM_USER_WAKE_FUTURE, None, None);
        }
    }
}

pub struct AsyncExecutor {
    pub tasks: Mutex<Vec<Pin<Box<dyn Future<Output = ()> + Send>>>>,
}

impl AsyncExecutor {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
        }
    }

    pub fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) {
        self.tasks.lock().push(Box::pin(fut));
    }

    pub fn poll_all(&self, hwnd: HWND) {
        let mut tasks = self.tasks.lock();
        let waker = Arc::new(WindowWaker { hwnd }).into();
        let mut cx = Context::from_waker(&waker);

        let mut i = 0;
        while i < tasks.len() {
            if tasks[i].as_mut().poll(&mut cx).is_ready() {
                tasks.remove(i);
            } else {
                i += 1;
            }
        }
    }
}
