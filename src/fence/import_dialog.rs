use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::window::{register_classname_ex, Base, BaseRef, Window};

const ID_LISTVIEW: u32 = 1001;
const ID_IMPORT_BTN: u32 = 1002;
const ID_CANCEL_BTN: u32 = 1003;

const COL_ICON: i32 = 0;
const COL_PATH: i32 = 1;
const COL_ACTION: i32 = 2;

pub const ACTION_KEEP: u32 = 0;
pub const ACTION_REMOVE: u32 = 1;

const DLG_DEFAULT_WIDTH: i32 = 600;
const DLG_DEFAULT_HEIGHT: i32 = 450;
const MARGIN: i32 = 10;
const BUTTON_WIDTH: i32 = 90;
const BUTTON_HEIGHT: i32 = 30;
const BOTTOM_PANEL_HEIGHT: i32 = 50;

#[derive(Clone)]
pub struct ImportItem {
    pub title: Arc<str>,
    pub path: Arc<str>,
    pub action: u32, // ACTION_KEEP or ACTION_REMOVE
}

struct ImportDialogInner {
    items: Vec<ImportItem>,
    himagelist: HIMAGELIST,
}

pub struct ImportDialog {
    base: BaseRef,
    inner: Mutex<ImportDialogInner>,
    on_import: Box<dyn Fn(Vec<ImportItem>) + Send + Sync + 'static>,
}

impl ImportDialog {
    pub fn create_window(
        items: Vec<ImportItem>,
        on_import: impl Fn(Vec<ImportItem>) + Send + Sync + 'static,
    ) -> Result<Arc<Self>> {
        let hinstance = unsafe { GetModuleHandleW(None).unwrap_or_default() };

        // Center dialog on screen
        let bounds = crate::app::App::get().screen_bounds();
        let screen_w = bounds.width.load(std::sync::atomic::Ordering::Relaxed);
        let screen_h = bounds.height.load(std::sync::atomic::Ordering::Relaxed);
        let dlg_w = DLG_DEFAULT_WIDTH;
        let dlg_h = DLG_DEFAULT_HEIGHT;
        let dlg_x = (screen_w - dlg_w) / 2;
        let dlg_y = (screen_h - dlg_h) / 2;

        let title_u16: Vec<u16> = "Import Icons"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        Base::create_window(
            WINDOW_EX_STYLE(0),
            register_classname_ex("FenceImportDialog", unsafe {
                let mut wc: WNDCLASSW = std::mem::zeroed();
                wc.hbrBackground = HBRUSH((COLOR_WINDOW.0 + 1) as *mut core::ffi::c_void);
                wc.hCursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
                wc
            }),
            PCWSTR(title_u16.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            dlg_x,
            dlg_y,
            dlg_w,
            dlg_h,
            HWND::default(),
            None,
            hinstance.into(),
            |base| {
                let hwnd = base.hwnd();
                let himagelist = unsafe {
                    ImageList_Create(32, 32, ILC_COLOR32 | ILC_MASK, items.len() as i32, 0)
                };

                // Create ListView
                let lv_hwnd = unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        w!("SysListView32"),
                        None,
                        WS_CHILD
                            | WS_VISIBLE
                            | WS_BORDER
                            | WINDOW_STYLE(LVS_REPORT | LVS_SINGLESEL),
                        0,
                        0,
                        0,
                        0,
                        Some(hwnd),
                        Some(HMENU(ID_LISTVIEW as *mut core::ffi::c_void)),
                        Some(hinstance.into()),
                        None,
                    )
                    .unwrap_or_default()
                };

                unsafe {
                    // Extended styles: full row select, subitem images
                    let _ = SendMessageW(
                        lv_hwnd,
                        LVM_SETEXTENDEDLISTVIEWSTYLE,
                        Some(WPARAM(0)),
                        Some(LPARAM(
                            (LVS_EX_FULLROWSELECT | LVS_EX_SUBITEMIMAGES) as isize,
                        )),
                    );

                    // Assign image list
                    let _ = SendMessageW(
                        lv_hwnd,
                        LVM_SETIMAGELIST,
                        Some(WPARAM(LVSIL_SMALL as usize)),
                        Some(LPARAM(himagelist.0 as isize)),
                    );

                    // Column 0: Icon
                    let col0_text: Vec<u16> = "".encode_utf16().chain(std::iter::once(0)).collect();
                    let mut col0: LVCOLUMNW = std::mem::zeroed();
                    col0.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
                    col0.cx = 40;
                    col0.pszText = windows::core::PWSTR(col0_text.as_ptr() as *mut _);
                    col0.iSubItem = COL_ICON;
                    let _ = SendMessageW(
                        lv_hwnd,
                        LVM_INSERTCOLUMNW,
                        Some(WPARAM(COL_ICON as usize)),
                        Some(LPARAM(&col0 as *const _ as isize)),
                    );

                    // Column 1: Path
                    let col1_text: Vec<u16> =
                        "Path".encode_utf16().chain(std::iter::once(0)).collect();
                    let mut col1: LVCOLUMNW = std::mem::zeroed();
                    col1.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
                    col1.cx = 380;
                    col1.pszText = windows::core::PWSTR(col1_text.as_ptr() as *mut _);
                    col1.iSubItem = COL_PATH;
                    let _ = SendMessageW(
                        lv_hwnd,
                        LVM_INSERTCOLUMNW,
                        Some(WPARAM(COL_PATH as usize)),
                        Some(LPARAM(&col1 as *const _ as isize)),
                    );

                    // Column 2: Action
                    let col2_text: Vec<u16> =
                        "Action".encode_utf16().chain(std::iter::once(0)).collect();
                    let mut col2: LVCOLUMNW = std::mem::zeroed();
                    col2.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
                    col2.cx = 140;
                    col2.pszText = windows::core::PWSTR(col2_text.as_ptr() as *mut _);
                    col2.iSubItem = COL_ACTION;
                    let _ = SendMessageW(
                        lv_hwnd,
                        LVM_INSERTCOLUMNW,
                        Some(WPARAM(COL_ACTION as usize)),
                        Some(LPARAM(&col2 as *const _ as isize)),
                    );
                }

                // Populate rows
                for (i, item) in items.iter().enumerate() {
                    // Load icon for this item
                    let icon_index = unsafe {
                        let path_u16: Vec<u16> =
                            item.path.encode_utf16().chain(std::iter::once(0)).collect();
                        let mut shfi: SHFILEINFOW = std::mem::zeroed();
                        SHGetFileInfoW(
                            PCWSTR(path_u16.as_ptr()),
                            FILE_FLAGS_AND_ATTRIBUTES(0),
                            Some(&mut shfi),
                            std::mem::size_of::<SHFILEINFOW>() as u32,
                            SHGFI_ICON | SHGFI_SMALLICON,
                        );
                        let idx = if !shfi.hIcon.is_invalid() {
                            ImageList_ReplaceIcon(himagelist, -1, shfi.hIcon)
                        } else {
                            let hicon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
                            ImageList_ReplaceIcon(himagelist, -1, hicon)
                        };
                        if !shfi.hIcon.is_invalid() {
                            let _ = DestroyIcon(shfi.hIcon);
                        }
                        idx
                    };

                    unsafe {
                        // Insert row with icon in column 0
                        let mut lvi: LVITEMW = std::mem::zeroed();
                        lvi.mask = LVIF_IMAGE;
                        lvi.iItem = i as i32;
                        lvi.iSubItem = COL_ICON;
                        lvi.iImage = icon_index;
                        let _ = SendMessageW(
                            lv_hwnd,
                            LVM_INSERTITEMW,
                            Some(WPARAM(0)),
                            Some(LPARAM(&lvi as *const _ as isize)),
                        );

                        // Column 1: path text
                        let path_u16: Vec<u16> =
                            item.path.encode_utf16().chain(std::iter::once(0)).collect();
                        let mut lvi_path: LVITEMW = std::mem::zeroed();
                        lvi_path.mask = LVIF_TEXT;
                        lvi_path.iItem = i as i32;
                        lvi_path.iSubItem = COL_PATH;
                        lvi_path.pszText = windows::core::PWSTR(path_u16.as_ptr() as *mut _);
                        let _ = SendMessageW(
                            lv_hwnd,
                            LVM_SETITEMW,
                            Some(WPARAM(0)),
                            Some(LPARAM(&lvi_path as *const _ as isize)),
                        );

                        // Column 2: action text
                        let action_str = if item.action == ACTION_KEEP {
                            "Keep"
                        } else {
                            "Remove"
                        };
                        let action_u16: Vec<u16> = action_str
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();
                        let mut lvi_action: LVITEMW = std::mem::zeroed();
                        lvi_action.mask = LVIF_TEXT;
                        lvi_action.iItem = i as i32;
                        lvi_action.iSubItem = COL_ACTION;
                        lvi_action.pszText = windows::core::PWSTR(action_u16.as_ptr() as *mut _);
                        let _ = SendMessageW(
                            lv_hwnd,
                            LVM_SETITEMW,
                            Some(WPARAM(0)),
                            Some(LPARAM(&lvi_action as *const _ as isize)),
                        );
                    }
                }

                // Import button
                crate::utils::create_button(
                    "Import",
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    Some(HMENU(ID_IMPORT_BTN as *mut core::ffi::c_void)),
                    hinstance.into(),
                );

                // Cancel button
                crate::utils::create_button(
                    "Cancel",
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    Some(HMENU(ID_CANCEL_BTN as *mut core::ffi::c_void)),
                    hinstance.into(),
                );

                let dialog = Arc::new(Self {
                    base,
                    inner: Mutex::new(ImportDialogInner { items, himagelist }),
                    on_import: Box::new(on_import),
                });

                dialog.layout_widgets();

                Ok(dialog)
            },
        )
    }

    fn get_listview_hwnd(&self) -> HWND {
        unsafe { GetDlgItem(Some(self.base.hwnd()), ID_LISTVIEW as i32).unwrap_or_default() }
    }

    fn get_import_btn_hwnd(&self) -> HWND {
        unsafe { GetDlgItem(Some(self.base.hwnd()), ID_IMPORT_BTN as i32).unwrap_or_default() }
    }

    fn get_cancel_btn_hwnd(&self) -> HWND {
        unsafe { GetDlgItem(Some(self.base.hwnd()), ID_CANCEL_BTN as i32).unwrap_or_default() }
    }

    fn layout_widgets(&self) {
        let hwnd = self.base.hwnd();
        let mut rect = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rect);
        };

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        let lv_hwnd = self.get_listview_hwnd();
        let import_btn_hwnd = self.get_import_btn_hwnd();
        let cancel_btn_hwnd = self.get_cancel_btn_hwnd();

        unsafe {
            // ListView takes most of the space
            let _ = MoveWindow(
                lv_hwnd,
                MARGIN,
                MARGIN,
                width - 2 * MARGIN,
                height - BOTTOM_PANEL_HEIGHT - MARGIN,
                true,
            );

            // Cancel button on the right
            let _ = MoveWindow(
                cancel_btn_hwnd,
                width - MARGIN - BUTTON_WIDTH,
                height - MARGIN - BUTTON_HEIGHT,
                BUTTON_WIDTH,
                BUTTON_HEIGHT,
                true,
            );

            // Import button to the left of Cancel
            let _ = MoveWindow(
                import_btn_hwnd,
                width - 2 * MARGIN - 2 * BUTTON_WIDTH,
                height - MARGIN - BUTTON_HEIGHT,
                BUTTON_WIDTH,
                BUTTON_HEIGHT,
                true,
            );
        }
    }

    /// Toggle the action of the selected row between Keep and Remove.
    fn toggle_selected_action(&self) {
        let lv = self.get_listview_hwnd();
        let sel = unsafe {
            SendMessageW(
                lv,
                LVM_GETNEXTITEM,
                Some(WPARAM(usize::MAX)),
                Some(LPARAM(LVNI_SELECTED as isize)),
            )
        };
        if sel.0 < 0 {
            return;
        }
        let idx = sel.0 as usize;
        let mut inner = self.inner.lock();
        if idx >= inner.items.len() {
            return;
        }
        let item = &mut inner.items[idx];
        item.action = if item.action == ACTION_KEEP {
            ACTION_REMOVE
        } else {
            ACTION_KEEP
        };
        let action_str = if item.action == ACTION_KEEP {
            "Keep"
        } else {
            "Remove"
        };
        let action_u16: Vec<u16> = action_str
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let mut lvi: LVITEMW = std::mem::zeroed();
            lvi.mask = LVIF_TEXT;
            lvi.iItem = idx as i32;
            lvi.iSubItem = COL_ACTION;
            lvi.pszText = windows::core::PWSTR(action_u16.as_ptr() as *mut _);
            let _ = SendMessageW(
                lv,
                LVM_SETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&lvi as *const _ as isize)),
            );
        }
    }

    fn do_import(&self) {
        let inner = self.inner.lock();
        let kept: Vec<ImportItem> = inner
            .items
            .iter()
            .filter(|i| i.action == ACTION_KEEP)
            .cloned()
            .collect();
        drop(inner);
        (self.on_import)(kept);
        unsafe {
            let _ = DestroyWindow(self.base.hwnd());
        };
    }
}

impl Window for ImportDialog {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_SIZE => {
                self.layout_widgets();
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u32;
                match id {
                    ID_IMPORT_BTN => {
                        self.do_import();
                        LRESULT(0)
                    }
                    ID_CANCEL_BTN => {
                        unsafe {
                            let _ = DestroyWindow(hwnd);
                        };
                        LRESULT(0)
                    }
                    _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
                }
            }
            WM_NOTIFY => {
                let nmhdr = unsafe { &*(lparam.0 as *const NMHDR) };
                if nmhdr.idFrom == ID_LISTVIEW as usize && nmhdr.code == NM_DBLCLK as u32 {
                    self.toggle_selected_action();
                    return LRESULT(0);
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_DESTROY => {
                let inner = self.inner.lock();
                unsafe {
                    let _ = ImageList_Destroy(Some(inner.himagelist));
                };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
