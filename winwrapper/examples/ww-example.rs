use std::sync::Arc;

use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use winwrapper::error::WinError;
use winwrapper::fut::AsyncExecutor;
use winwrapper::layout::{Item, Layout, Orientation};
use winwrapper::mutex::Mutex;
use winwrapper::utils::HWNDWrapper;
use winwrapper::window::{register_classname, Base, BaseRef, Window};
use winwrapper::{controls, prompt};

type Result<T> = std::result::Result<T, WinError>;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const WM_USER_WAKE_FUTURE: u32 = WM_USER + 1;
type Exec = AsyncExecutor<{ WM_USER_WAKE_FUTURE }>;

const ID_BTN_MESSAGE: usize = 1001;
const ID_BTN_INPUT: usize = 1002;
const ID_BTN_FOLDER: usize = 1003;

const WIDTH: i32 = 360;
const HEIGHT: i32 = 200;
const BTN_H: i32 = 32;

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    base: BaseRef,
    executor: Exec,
    layout: Mutex<Layout>,
}

impl App {
    fn new() -> Result<Arc<Self>> {
        let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

        // Register window class
        let class = register_classname("WWExampleClass");

        // Create the window via Base
        Base::create_window::<Self, _>(
            0,
            class,
            w!("winwrapper example"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            (unsafe { GetSystemMetrics(SM_CXSCREEN) } - WIDTH) / 2,
            (unsafe { GetSystemMetrics(SM_CYSCREEN) } - HEIGHT) / 2,
            WIDTH,
            HEIGHT,
            HWND::default(),
            None,
            hinstance.into(),
            |base| {
                let hwnd = base.hwnd();

                // Create buttons
                let btn_msg = controls::create_button(
                    "Test Message",
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    Some(ID_BTN_MESSAGE as HMENU),
                    hinstance.into(),
                );
                let btn_input = controls::create_button(
                    "Input Text",
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    Some(ID_BTN_INPUT as HMENU),
                    hinstance.into(),
                );
                let btn_folder = controls::create_button(
                    "Select Folder",
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    Some(ID_BTN_FOLDER as HMENU),
                    hinstance.into(),
                );

                let layout = Layout {
                    orientation: Orientation::Vertical,
                    items: vec![
                        Item::Fixed {
                            hwnd: HWNDWrapper(btn_msg),
                            size: BTN_H,
                        },
                        Item::Fixed {
                            hwnd: HWNDWrapper(btn_input),
                            size: BTN_H,
                        },
                        Item::Fixed {
                            hwnd: HWNDWrapper(btn_folder),
                            size: BTN_H,
                        },
                    ],
                    ..Default::default()
                };

                let app = Arc::new(Self {
                    base,
                    executor: Exec::new(HWNDWrapper(hwnd)),
                    layout: Mutex::new(layout),
                });

                // Initial layout
                app.layout_widgets();
                Ok(app)
            },
        )
    }

    fn layout_widgets(&self) {
        let mut rect = RECT::default();
        unsafe {
            let _ = GetClientRect(self.base.hwnd(), &mut rect);
        }
        let layout = self.layout.lock();
        layout.arrange(rect);
    }

    fn on_command(&self, wparam: WPARAM) {
        let id = (wparam as u32 & 0xFFFF) as usize;

        match id {
            ID_BTN_MESSAGE => {
                self.executor.spawn(async {
                    let _ = prompt::confirm(
                        "Hello! This is a test message.",
                        "Test Message",
                        MB_OK | MB_ICONINFORMATION,
                    )
                    .await;
                });
            }

            ID_BTN_INPUT => {
                self.executor.spawn(async {
                    let result =
                        prompt::input("Enter something", "Type some text below:", "default text")
                            .await;
                    let (text, caption) = match result {
                        Some(s) if !s.is_empty() => (format!("You entered:\n{s}"), "Input Result"),
                        _ => ("No input was provided.".into(), "Input Result"),
                    };
                    let _ = prompt::confirm(&text, caption, MB_OK | MB_ICONINFORMATION).await;
                });
            }

            ID_BTN_FOLDER => {
                self.executor.spawn(async {
                    let path = prompt::browse_for_folder().await;
                    let (text, caption) = match path {
                        Some(p) => (format!("Selected folder:\n{p}"), "Folder Result"),
                        None => ("No folder was selected.".into(), "Folder Result"),
                    };
                    let _ = prompt::confirm(&text, caption, MB_OK | MB_ICONINFORMATION).await;
                });
            }

            _ => {}
        }
    }
}

impl Window for App {
    fn base(&self) -> &BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
        match msg {
            WM_SIZE => {
                self.layout_widgets();
                0
            }
            WM_CLOSE => {
                unsafe {
                    let _ = DestroyWindow(self.base.hwnd());
                }
                0
            }
            WM_DESTROY => {
                unsafe {
                    PostQuitMessage(0);
                }
                0
            }
            WM_USER_WAKE_FUTURE => {
                self.executor.poll_all();
                0
            }
            WM_COMMAND => {
                self.on_command(wparam);
                0
            }
            _ => unsafe { DefWindowProcW(self.base.hwnd(), msg, wparam, lparam) },
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    unsafe {
        // Initialise common controls so that standard windows (buttons, …) work
        let mut icc: INITCOMMONCONTROLSEX = std::mem::zeroed();
        icc.dwSize = std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32;
        icc.dwICC = ICC_STANDARD_CLASSES;
        InitCommonControlsEx(&icc);
    }

    let _app = App::new()?;

    unsafe {
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
