use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::*;

use crate::layout::{Item, Layout, Orientation};
use crate::utils::HWNDWrapper;
use crate::window::{Base, BaseRef, Window, register_classname_ex};

const ID_LISTVIEW: u32 = 1001;
const ID_IMPORT_BTN: u32 = 1002;
const ID_CANCEL_BTN: u32 = 1003;
const ID_SHOW_LNK_ONLY: u32 = 1004;

const COL_ICON: i32 = 0;
const COL_PATH: i32 = 1;
const COL_ACTION: i32 = 2;

pub const ACTION_KEEP: u32 = 0;
pub const ACTION_REMOVE: u32 = 1;

const DLG_DEFAULT_WIDTH: i32 = 600;
const DLG_DEFAULT_HEIGHT: i32 = 450;
const BUTTON_WIDTH: i32 = 90;
const BUTTON_HEIGHT: i32 = 30;

#[derive(Clone)]
pub struct ImportItem {
    pub title: Arc<str>,
    pub path: Arc<str>,
    pub action: u32, // ACTION_KEEP or ACTION_REMOVE
}

struct ImportDialogInner {
    items: Vec<ImportItem>,
    // Maps list view row indices to inner.items indices, required because
    // the list view may show a filtered subset (e.g. only .lnk files)
    visible_indices: Vec<usize>,
    show_lnk_only: bool,
    himagelist: HIMAGELIST,
    layout: Layout,
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
        let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

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
            0,
            register_classname_ex("FenceImportDialog", unsafe {
                let mut wc: WNDCLASSW = std::mem::zeroed();
                wc.hbrBackground = (COLOR_WINDOW + 1) as HBRUSH;
                wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
                wc
            }),
            title_u16.as_ptr(),
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
                        0,
                        w!("SysListView32"),
                        std::ptr::null(),
                        WS_CHILD | WS_VISIBLE | WS_BORDER | (LVS_REPORT | LVS_SINGLESEL),
                        0,
                        0,
                        0,
                        0,
                        hwnd,
                        ID_LISTVIEW as HMENU,
                        hinstance,
                        std::ptr::null(),
                    )
                };

                unsafe {
                    // Extended styles: full row select, subitem images
                    let _ = SendMessageW(
                        lv_hwnd,
                        LVM_SETEXTENDEDLISTVIEWSTYLE,
                        0 as WPARAM,
                        (LVS_EX_FULLROWSELECT | LVS_EX_SUBITEMIMAGES) as LPARAM,
                    );

                    // Assign image list
                    let _ = SendMessageW(
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
                    let _ = SendMessageW(
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
                    let _ = SendMessageW(
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
                    let _ = SendMessageW(
                        lv_hwnd,
                        LVM_INSERTCOLUMNW,
                        COL_ACTION as WPARAM,
                        &col2 as *const _ as LPARAM,
                    );
                }

                // Import button
                let import_btn = crate::controls::create_button(
                    "Import",
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    Some(ID_IMPORT_BTN as HMENU),
                    hinstance.into(),
                );

                // Cancel button
                let cancel_btn = crate::controls::create_button(
                    "Cancel",
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    Some(ID_CANCEL_BTN as HMENU),
                    hinstance.into(),
                );

                // Checkbox for LNK filter
                let lnk_checkbox = crate::controls::create_checkbox(
                    "Show only shortcuts (.LNK)",
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    Some(ID_SHOW_LNK_ONLY as HMENU),
                    hinstance.into(),
                );

                let layout = Layout {
                    orientation: Orientation::Vertical,
                    items: vec![
                        Item::Fixed {
                            hwnd: HWNDWrapper(lnk_checkbox),
                            size: 22,
                        },
                        Item::Fill {
                            hwnd: HWNDWrapper(lv_hwnd),
                            min: 0,
                        },
                        Item::Nested {
                            layout: Box::new(Layout {
                                orientation: Orientation::Horizontal,
                                items: vec![
                                    Item::Fill {
                                        hwnd: HWNDWrapper(HWND::default()),
                                        min: 0,
                                    },
                                    Item::Fixed {
                                        hwnd: HWNDWrapper(import_btn),
                                        size: BUTTON_WIDTH,
                                    },
                                    Item::Fixed {
                                        hwnd: HWNDWrapper(cancel_btn),
                                        size: BUTTON_WIDTH,
                                    },
                                ],
                                margin: 0,
                                ..Default::default()
                            }),
                            size: BUTTON_HEIGHT,
                        },
                    ],
                    ..Default::default()
                };

                let dialog = Arc::new(Self {
                    base,
                    inner: Mutex::new(ImportDialogInner {
                        items,
                        visible_indices: Vec::new(),
                        show_lnk_only: false,
                        himagelist,
                        layout,
                    }),
                    on_import: Box::new(on_import),
                });

                dialog.populate_listview();
                dialog.layout_widgets();

                Ok(dialog)
            },
        )
    }

    fn get_listview_hwnd(&self) -> HWND {
        unsafe { GetDlgItem(self.base.hwnd(), ID_LISTVIEW as i32) }
    }

    fn layout_widgets(&self) {
        let hwnd = self.base.hwnd();
        let mut rect = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rect);
        };
        let inner = self.inner.lock();
        inner.layout.arrange(rect.clone());
    }

    fn populate_listview(&self) {
        let lv = self.get_listview_hwnd();
        let mut inner = self.inner.lock();

        // Clear existing items
        unsafe {
            let _ = SendMessageW(lv, LVM_DELETEALLITEMS, 0 as WPARAM, 0 as LPARAM);
        }

        // Build filtered visible indices
        let show_lnk_only = inner.show_lnk_only;
        let mut visible = Vec::new();
        for (i, item) in inner.items.iter().enumerate() {
            let show = if show_lnk_only {
                item.path.to_lowercase().ends_with(".lnk")
            } else {
                true
            };
            if show {
                visible.push(i);
            }
        }
        inner.visible_indices = visible;

        // Load icons for visible items
        let himagelist = inner.himagelist;
        for &item_idx in &inner.visible_indices {
            let item = &inner.items[item_idx];
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
                let idx = if shfi.hIcon != std::ptr::null_mut() {
                    ImageList_ReplaceIcon(himagelist, -1, shfi.hIcon)
                } else {
                    let hicon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
                    ImageList_ReplaceIcon(himagelist, -1, hicon)
                };
                if shfi.hIcon != std::ptr::null_mut() {
                    let _ = DestroyIcon(shfi.hIcon);
                }
                idx
            };

            let row = inner
                .visible_indices
                .iter()
                .position(|&x| x == item_idx)
                .unwrap() as i32;
            unsafe {
                // Insert row with icon in column 0
                let mut lvi: LVITEMW = std::mem::zeroed();
                lvi.mask = LVIF_IMAGE;
                lvi.iItem = row;
                lvi.iSubItem = COL_ICON;
                lvi.iImage = icon_index;
                let _ = SendMessageW(lv, LVM_INSERTITEMW, 0 as WPARAM, &lvi as *const _ as LPARAM);

                // Column 1: path text
                let path_u16: Vec<u16> =
                    item.path.encode_utf16().chain(std::iter::once(0)).collect();
                let mut lvi_path: LVITEMW = std::mem::zeroed();
                lvi_path.mask = LVIF_TEXT;
                lvi_path.iItem = row;
                lvi_path.iSubItem = COL_PATH;
                lvi_path.pszText = path_u16.as_ptr() as *mut _;
                let _ = SendMessageW(
                    lv,
                    LVM_SETITEMW,
                    0 as WPARAM,
                    &lvi_path as *const _ as LPARAM,
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
                lvi_action.iItem = row;
                lvi_action.iSubItem = COL_ACTION;
                lvi_action.pszText = action_u16.as_ptr() as *mut _;
                let _ = SendMessageW(
                    lv,
                    LVM_SETITEMW,
                    0 as WPARAM,
                    &lvi_action as *const _ as LPARAM,
                );
            }
        }
    }

    /// Toggle the action of the selected row between Keep and Remove.
    fn toggle_selected_action(&self) {
        let lv = self.get_listview_hwnd();
        let sel = unsafe {
            SendMessageW(
                lv,
                LVM_GETNEXTITEM,
                usize::MAX as WPARAM,
                LVNI_SELECTED as LPARAM,
            )
        };
        if sel < 0 {
            return;
        }
        let visible_idx = sel as usize;
        let mut inner = self.inner.lock();
        if visible_idx >= inner.visible_indices.len() {
            return;
        }
        let item_idx = inner.visible_indices[visible_idx];
        let item = &mut inner.items[item_idx];
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
            lvi.iItem = visible_idx as i32;
            lvi.iSubItem = COL_ACTION;
            lvi.pszText = action_u16.as_ptr() as *mut _;
            let _ = SendMessageW(lv, LVM_SETITEMW, 0 as WPARAM, &lvi as *const _ as LPARAM);
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
                0
            }
            WM_COMMAND => {
                let id = (wparam & 0xFFFF) as u32;
                match id {
                    ID_IMPORT_BTN => {
                        self.do_import();
                        0
                    }
                    ID_CANCEL_BTN => {
                        unsafe {
                            let _ = DestroyWindow(hwnd);
                        };
                        0
                    }
                    ID_SHOW_LNK_ONLY => {
                        let hi = ((wparam as u32) >> 16) as u16;
                        tracing::debug!("SHOW_LNK_ONLY hi={}", hi);
                        if hi == BN_CLICKED as u16 {
                            let mut inner = self.inner.lock();
                            inner.show_lnk_only = !inner.show_lnk_only;
                            tracing::debug!("show_lnk_only toggled to {}", inner.show_lnk_only);
                            drop(inner);
                            self.populate_listview();
                        }
                        0
                    }
                    _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
                }
            }
            WM_NOTIFY => {
                let nmhdr = unsafe { &*(lparam as *const NMHDR) };
                if nmhdr.idFrom == ID_LISTVIEW as usize && nmhdr.code == NM_DBLCLK as u32 {
                    self.toggle_selected_action();
                    return 0;
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_DESTROY => {
                let inner = self.inner.lock();
                unsafe {
                    let _ = ImageList_Destroy(inner.himagelist);
                };
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
