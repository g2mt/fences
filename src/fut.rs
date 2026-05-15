use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use parking_lot::Mutex;
use windows_sys::Win32::Foundation::{LPARAM, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::desktop_cover::WM_USER_WAKE_FUTURE;
use crate::utils::HWNDWrapper;

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
    hwnd_w: HWNDWrapper,
}

impl std::task::Wake for WindowWaker {
    fn wake(self: Arc<Self>) {
        unsafe {
            let _ = PostMessageW(self.hwnd_w.0, WM_USER_WAKE_FUTURE, 0 as WPARAM, 0 as LPARAM);
        }
    }
}

pub struct AsyncExecutor {
    pub tasks: Mutex<Vec<Pin<Box<dyn Future<Output = ()> + Send>>>>,
    hwnd_w: HWNDWrapper,
}

impl AsyncExecutor {
    pub fn new(hwnd_w: HWNDWrapper) -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            hwnd_w,
        }
    }

    pub fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) {
        self.tasks.lock().push(Box::pin(fut));
        unsafe {
            let _ = PostMessageW(self.hwnd_w.0, WM_USER_WAKE_FUTURE, 0, 0);
        }
    }

    pub fn poll_all(&self) {
        let mut tasks = self.tasks.lock();
        let waker = Arc::new(WindowWaker {
            hwnd_w: self.hwnd_w,
        })
        .into();
        let mut cx = Context::from_waker(&waker);
        tasks.retain_mut(|task| !task.as_mut().poll(&mut cx).is_ready());
    }
}
