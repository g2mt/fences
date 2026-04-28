use std::sync::{Arc, Mutex};

use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::app::App;
use crate::window::{register_classname, Base, BaseRef, Window};

const ID_LISTVIEW: u32 = 1001;
const ID_IMPORT_BTN: u32 = 1002;
const ID_CANCEL_BTN: u32 = 1003;

const COL_ICON: i32 = 0;
const COL_PATH: i32 = 1;
const COL_ACTION: i32 = 2;

pub const ACTION_KEEP: u32 = 0;
pub const ACTION_REMOVE: u32 = 1;

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
    pub fn show(
        parent_hwnd: HWND,
        items: Vec<ImportItem>,
        on_import: impl Fn(Vec<ImportItem>) + Send + Sync + 'static,
    ) {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };

        // Center dialog on screen
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let dlg_w = 600;
        let dlg_h = 450;
        let dlg_x = (screen_w - dlg_w) / 2;
        let dlg_y = (screen_h - dlg_h) / 2;

        let title_u16: Vec<u16> = "Import Icons"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        Base::create_window(
            WS_EX_DLGMODALFRAME | WS_EX_TOPMOST,
            register_classname(w!("FenceImportDialog")),
            title_u16.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_CLIPCHILDREN,
            dlg_x,
            dlg_y,
            dlg_w,
            dlg_h,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
            |base| {
                let hwnd = base.hwnd();
                let himagelist = unsafe {
                    ImageList_Create(32, 32, ILC_COLOR32 | ILC_MASK, items.len() as i32, 0)
                };

                // Create ListView
                let lv_hwnd = unsafe {
                    CreateWindowExW(
                        0,
                        w!("SysListView32"),
                        std::ptr::null(),
                        WS_CHILD | WS_VISIBLE | WS_BORDER | LVS_REPORT | LVS_SINGLESEL,
                        10,
                        10,
                        dlg_w - 20,
                        dlg_h - 60,
                        hwnd,
                        ID_LISTVIEW as _,
                        h_instance,
                        std::ptr::null_mut(),
                    )
                };

                unsafe {
                    // Extended styles: full row select, subitem images
                    SendMessageW(
                        lv_hwnd,
                        LVM_SETEXTENDEDLISTVIEWSTYLE,
                        0,
                        (LVS_EX_FULLROWSELECT | LVS_EX_SUBITEMIMAGES) as LPARAM,
                    );

                    // Assign image list
                    SendMessageW(
                        lv_hwnd,
                        LVM_SETIMAGELIST,
                        LVSIL_SMALL as WPARAM,
                        himagelist as LPARAM,
                    );

                    // Column 0: Icon
                    let col0_text: Vec<u16> = "".encode_utf16().chain(std::iter::once(0)).collect();
                    let mut col0: LVCOLUMNW = std::mem::zeroed();
                    col0.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
                    col0.cx = 40;
                    col0.pszText = col0_text.as_ptr() as *mut _;
                    col0.iSubItem = COL_ICON;
                    SendMessageW(
                        lv_hwnd,
                        LVM_INSERTCOLUMNW,
                        COL_ICON as WPARAM,
                        &col0 as *const _ as LPARAM,
                    );

                    // Column 1: Path
                    let col1_text: Vec<u16> =
                        "Path".encode_utf16().chain(std::iter::once(0)).collect();
                    let mut col1: LVCOLUMNW = std::mem::zeroed();
                    col1.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
                    col1.cx = 380;
                    col1.pszText = col1_text.as_ptr() as *mut _;
                    col1.iSubItem = COL_PATH;
                    SendMessageW(
                        lv_hwnd,
                        LVM_INSERTCOLUMNW,
                        COL_PATH as WPARAM,
                        &col1 as *const _ as LPARAM,
                    );

                    // Column 2: Action
                    let col2_text: Vec<u16> =
                        "Action".encode_utf16().chain(std::iter::once(0)).collect();
                    let mut col2: LVCOLUMNW = std::mem::zeroed();
                    col2.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
                    col2.cx = 140;
                    col2.pszText = col2_text.as_ptr() as *mut _;
                    col2.iSubItem = COL_ACTION;
                    SendMessageW(
                        lv_hwnd,
                        LVM_INSERTCOLUMNW,
                        COL_ACTION as WPARAM,
                        &col2 as *const _ as LPARAM,
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
                            path_u16.as_ptr(),
                            0,
                            &mut shfi,
                            std::mem::size_of::<SHFILEINFOW>() as u32,
                            SHGFI_ICON | SHGFI_SMALLICON,
                        );
                        let idx = if !shfi.hIcon.is_null() {
                            ImageList_ReplaceIcon(himagelist, -1, shfi.hIcon)
                        } else {
                            let hicon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
                            ImageList_ReplaceIcon(himagelist, -1, hicon)
                        };
                        if !shfi.hIcon.is_null() {
                            DestroyIcon(shfi.hIcon);
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
                        SendMessageW(lv_hwnd, LVM_INSERTITEMW, 0, &lvi as *const _ as LPARAM);

                        // Column 1: path text
                        let path_u16: Vec<u16> =
                            item.path.encode_utf16().chain(std::iter::once(0)).collect();
                        let mut lvi_path: LVITEMW = std::mem::zeroed();
                        lvi_path.mask = LVIF_TEXT;
                        lvi_path.iItem = i as i32;
                        lvi_path.iSubItem = COL_PATH;
                        lvi_path.pszText = path_u16.as_ptr() as *mut _;
                        SendMessageW(lv_hwnd, LVM_SETITEMW, 0, &lvi_path as *const _ as LPARAM);

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
                        lvi_action.pszText = action_u16.as_ptr() as *mut _;
                        SendMessageW(lv_hwnd, LVM_SETITEMW, 0, &lvi_action as *const _ as LPARAM);
                    }
                }

                // Import button
                let import_text: Vec<u16> =
                    "Import".encode_utf16().chain(std::iter::once(0)).collect();
                unsafe {
                    CreateWindowExW(
                        0,
                        w!("BUTTON"),
                        import_text.as_ptr(),
                        WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32),
                        dlg_w - 200,
                        dlg_h - 40,
                        90,
                        30,
                        hwnd,
                        ID_IMPORT_BTN as _,
                        h_instance,
                        std::ptr::null_mut(),
                    );
                }

                // Cancel button
                let cancel_text: Vec<u16> =
                    "Cancel".encode_utf16().chain(std::iter::once(0)).collect();
                unsafe {
                    CreateWindowExW(
                        0,
                        w!("BUTTON"),
                        cancel_text.as_ptr(),
                        WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32),
                        dlg_w - 100,
                        dlg_h - 40,
                        90,
                        30,
                        hwnd,
                        ID_CANCEL_BTN as _,
                        h_instance,
                        std::ptr::null_mut(),
                    );
                }

                Ok(Arc::new(Self {
                    base,
                    inner: Mutex::new(ImportDialogInner { items, himagelist }),
                    on_import: Box::new(on_import),
                }))
            },
        )
        .expect("Failed to create ImportDialog window");
    }

    fn get_listview_hwnd(&self) -> HWND {
        unsafe { GetDlgItem(self.base.hwnd(), ID_LISTVIEW as i32) }
    }

    /// Toggle the action of the selected row between Keep and Remove.
    fn toggle_selected_action(&self) {
        let lv = self.get_listview_hwnd();
        let sel = unsafe { SendMessageW(lv, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as LPARAM) };
        if sel < 0 {
            return;
        }
        let idx = sel as usize;
        let mut inner = self.inner.lock().unwrap();
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
            lvi.pszText = action_u16.as_ptr() as *mut _;
            SendMessageW(lv, LVM_SETITEMW, 0, &lvi as *const _ as LPARAM);
        }
    }

    fn do_import(&self) {
        let inner = self.inner.lock().unwrap();
        let kept: Vec<ImportItem> = inner
            .items
            .iter()
            .filter(|i| i.action == ACTION_KEEP)
            .cloned()
            .collect();
        drop(inner);
        (self.on_import)(kept);
        unsafe { DestroyWindow(self.base.hwnd()) };
    }
}

impl Window for ImportDialog {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_COMMAND => {
                let id = (wparam & 0xFFFF) as u32;
                match id {
                    ID_IMPORT_BTN => {
                        self.do_import();
                        0
                    }
                    ID_CANCEL_BTN => {
                        unsafe { DestroyWindow(hwnd) };
                        0
                    }
                    _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
                }
            }
            WM_NOTIFY => {
                let nmhdr = unsafe { &*(lparam as *const NMHDR) };
                if nmhdr.idFrom == ID_LISTVIEW as usize && nmhdr.code == NM_DBLCLK {
                    self.toggle_selected_action();
                    return 0;
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                let config = App::config();
                config.fence.fence_bg_color.paint_background(hdc, &rect);
                EndPaint(hwnd, &ps);
                0
            },
            WM_DESTROY => {
                let inner = self.inner.lock().unwrap();
                unsafe { ImageList_Destroy(inner.himagelist) };
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
