use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use anyhow::Result;
use tracing::{debug, error};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, DragQueryPoint, ShellExecuteW, HDROP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use winwrapper::geo::Area;
use winwrapper::mutex::Mutex;
use winwrapper::prompt;
use winwrapper::window::{register_classname, Base, BaseRef, Window};

use crate::app::App;
use crate::commands::*;
use crate::config::state::{FenceState, FenceStickyPosition, IconState};
use crate::desktop_cover::DesktopCover;
use crate::fence::icon::Icon;
use crate::fence::import_dialog::{ImportDialog, ImportItem};
use crate::fence::scroll_area::ScrollArea;
use crate::fence::title_bar::TitleBar;

// Custom events
pub const WM_USER_PAINT_WITH_ALPHA: u32 = WM_USER + 1;

#[derive(Clone, PartialEq, Debug)]
pub enum Hit {
    TitleBar,
    Client,
    Icon(Arc<Icon>),
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Hit {
    fn from_pos_in_fence(fence: &Fence, rel_x: i32, rel_y: i32) -> Option<Self> {
        let config = App::config();
        let border = config.fence.border_thickness;
        let title_h = config.fence.title_bar_height;

        let area = fence.base().area();
        let on_left = rel_x < border;
        let on_right = rel_x >= area.width.load(Ordering::Relaxed) - border;
        let on_top = rel_y < border;
        let on_bottom = rel_y >= area.height.load(Ordering::Relaxed) - border;

        let hit = match (on_left, on_right, on_top, on_bottom) {
            (true, _, true, _) => Self::TopLeft,
            (_, true, true, _) => Self::TopRight,
            (true, _, _, true) => Self::BottomLeft,
            (_, true, _, true) => Self::BottomRight,
            (true, _, _, _) => Self::Left,
            (_, true, _, _) => Self::Right,
            (_, _, true, _) => Self::Top,
            (_, _, _, true) => Self::Bottom,
            _ => {
                if rel_y < title_h {
                    Self::TitleBar
                } else {
                    let scroll_area_rel_x = rel_x - border;
                    let scroll_area_rel_y = rel_y - title_h;
                    fence
                        .scroll_area
                        .icon_by_pos(scroll_area_rel_x, scroll_area_rel_y)
                        .map(Self::Icon)
                        .unwrap_or(Self::Client)
                }
            }
        };
        Some(hit)
    }
}

pub struct HitManager {
    m: Mutex<Option<Hit>>,
}

/// Implements "Hit" detection for the Fence.
/// Apart from mouse movement and cursor changes, all event handlers
/// will set the Hit value using update_hit.
impl HitManager {
    fn new() -> Self {
        Self {
            m: Mutex::new(None),
        }
    }

    /// Updates the Hit value based on relative mouse position, returning the copied Hit value
    pub fn update_hit(&self, fence: &Fence, rel_x: i32, rel_y: i32) -> Option<Hit> {
        let hit = Hit::from_pos_in_fence(fence, rel_x, rel_y);

        if *self.m.lock() != hit {
            let icon = hit.as_ref().and_then(|h| match h {
                Hit::Icon(icon) => Some(icon),
                _ => None,
            });
            fence.scroll_area.select_icon(icon);
            *self.m.lock() = hit.clone();
        }

        hit
    }

    /// Handles changing the Hit value from left mouse button down. Returns true if mouse is
    /// captured
    pub fn on_lbutton_down(&self, fence: &Fence, rel_x: i32, rel_y: i32) -> bool {
        match self.update_hit(fence, rel_x, rel_y) {
            None | Some(Hit::Icon(_)) | Some(Hit::Client) => false,
            _ => true,
        }
    }

    /// Unsets the Hit value from left mouse button up
    pub fn on_lbutton_up(&self, _fence: &Fence, _rel_x: i32, _rel_y: i32) {
        *self.m.lock() = None;
    }

    /// Runs the currently selected icon on double click
    pub fn on_lbutton_dblclk(&self, fence: &Fence, rel_x: i32, rel_y: i32) {
        if let Some(Hit::Icon(icon)) = Hit::from_pos_in_fence(fence, rel_x, rel_y) {
            icon.run();
        }
    }

    /// Activates context menu for either Fence or Icon based on the current Hit value
    /// THis also unsets the current Hit value
    pub fn on_rbutton_up(&self, fence: &Fence, rel_x: i32, rel_y: i32) {
        let hit = self.update_hit(fence, rel_x, rel_y);
        let mut pt = POINT { x: rel_x, y: rel_y };
        unsafe {
            let _ = ClientToScreen(fence.base().hwnd(), &mut pt);
        }

        if let Some(Hit::Icon(icon)) = hit {
            icon.show_context_menu(pt.x, pt.y, fence.base.hwnd());
        } else {
            fence.show_context_menu(pt.x, pt.y);
        }
    }

    /// Returns the cursor at that specific location for
    /// the current Hit value, or IDC_ARROW if out of bounds
    pub fn on_set_cursor(&self, fence: &Fence, rel_x: i32, rel_y: i32) -> Option<HCURSOR> {
        let hit = Hit::from_pos_in_fence(fence, rel_x, rel_y);
        let cursor_id = match hit {
            None => return None,
            Some(Hit::TitleBar) => IDC_SIZEALL,
            Some(Hit::Left) | Some(Hit::Right) => IDC_SIZEWE,
            Some(Hit::Top) | Some(Hit::Bottom) => IDC_SIZENS,
            Some(Hit::TopLeft) | Some(Hit::BottomRight) => IDC_SIZENWSE,
            Some(Hit::TopRight) | Some(Hit::BottomLeft) => IDC_SIZENESW,
            _ => IDC_ARROW,
        };
        Some(unsafe { LoadCursorW(std::ptr::null_mut(), cursor_id) })
    }

    /// Reacts based on the dragging movement of the mouse
    pub fn on_mouse_move(&self, fence: &Fence, dx: i32, dy: i32) {
        // Clone to ensure hit manager doesnt deadlock
        if let Some(hit_type) = self.m.lock().clone() {
            match hit_type {
                Hit::TitleBar => fence.base().move_by(dx, dy),
                Hit::Left => fence.add_area(dx, 0, -dx, 0),
                Hit::Right => fence.add_area(0, 0, dx, 0),
                Hit::Top => fence.add_area(0, dy, 0, -dy),
                Hit::Bottom => fence.add_area(0, 0, 0, dy),
                Hit::TopLeft => fence.add_area(dx, dy, -dx, -dy),
                Hit::TopRight => fence.add_area(0, dy, dx, -dy),
                Hit::BottomLeft => fence.add_area(dx, 0, -dx, dy),
                Hit::BottomRight => fence.add_area(0, 0, dx, dy),
                Hit::Client | Hit::Icon(_) => return,
            }

            fence.base().redraw(true);
            App::get().save_thread.get().unwrap().set_unsaved();
        }
    }
}

pub struct Fence {
    self_weak: Weak<Fence>,
    base: BaseRef,
    title_bar: Arc<TitleBar>,
    scroll_area: Arc<ScrollArea>,
    imported_from: Mutex<Option<Arc<str>>>,
    sticky_pos: Mutex<Option<FenceStickyPosition>>,
    last_mouse_pos: Mutex<POINT>,
    hitman: HitManager,
}

impl Fence {
    pub fn new(cover: &DesktopCover, x: i32, y: i32, title: &str) -> Result<Arc<Self>> {
        Self::from_state(
            cover,
            FenceState {
                title: Arc::from(title),
                area: Area::new(x, y, 300, 150),
                icons: Vec::new(),
                imported_from: None,
                sticky_pos: None,
            },
        )
    }

    pub fn from_state(cover: &DesktopCover, state: FenceState) -> Result<Arc<Self>> {
        let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let use_layered = App::config().use_layered_window;
        let parent_hwnd = if use_layered {
            App::get().hwnd_shell.get().unwrap().0
        } else {
            cover.base().hwnd()
        };
        debug!("parent_hwnd={:?}", parent_hwnd);
        Base::create_window(
            if use_layered { WS_EX_LAYERED } else { 0 },
            register_classname("Fence"),
            std::ptr::null(),
            WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            state.area.x,
            state.area.y,
            state.area.width,
            state.area.height,
            parent_hwnd,
            None,
            hinstance.into(),
            |base| {
                let title_bar = TitleBar::new(base.hwnd(), state.title.clone(), &state.area)?;
                let scroll_area = ScrollArea::new(base.hwnd(), &state.area)?;

                let fence = Arc::new_cyclic(|self_weak| Self {
                    self_weak: self_weak.clone(),
                    base,
                    title_bar,
                    scroll_area,
                    imported_from: Mutex::new(state.imported_from.clone()),
                    sticky_pos: Mutex::new(state.sticky_pos),
                    last_mouse_pos: Mutex::new(POINT { x: 0, y: 0 }),
                    hitman: HitManager::new(),
                });
                fence
                    .scroll_area
                    .add_icons_from_state(state.icons.iter(), true);
                unsafe { DragAcceptFiles(fence.base().hwnd(), 1) };
                if use_layered {
                    fence.paint_with_alpha();
                    unsafe {
                        let _ = SetWindowPos(
                            fence.base().hwnd(),
                            HWND_BOTTOM,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                }
                Ok(fence)
            },
        )
    }

    pub fn state(&self) -> FenceState {
        let area = self.base.area();
        FenceState {
            title: self.title_bar.title(),
            area: Area::new(
                area.x.load(Ordering::Relaxed),
                area.y.load(Ordering::Relaxed),
                area.width.load(Ordering::Relaxed),
                area.height.load(Ordering::Relaxed),
            ),
            icons: self.scroll_area.state(),
            imported_from: self.imported_from().clone(),
            sticky_pos: self.sticky(),
        }
    }

    pub fn title(&self) -> Arc<str> {
        self.title_bar.title()
    }

    pub fn set_title(&self, title: Arc<str>) {
        self.title_bar.set_title(title);
    }

    pub fn sticky(&self) -> Option<crate::config::state::FenceStickyPosition> {
        *self.sticky_pos.lock()
    }

    pub fn set_sticky(&self, sticky: Option<crate::config::state::FenceStickyPosition>) {
        *self.sticky_pos.lock() = sticky;
    }

    pub fn imported_from(&self) -> Option<Arc<str>> {
        self.imported_from.lock().clone()
    }

    pub fn set_imported_from(&self, imported_from: Option<Arc<str>>) {
        *self.imported_from.lock() = imported_from;
    }

    pub fn show_import_existing_dialog(self: &Arc<Self>) {
        App::get().import_dialog.lock().take();
        let imported_from = if let Some(p) = self.imported_from() {
            p
        } else {
            return;
        };

        let folder_path = Path::new(imported_from.as_ref());

        // Read all files from the directory: path -> title
        let mut dir_map: HashMap<String, String> = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(folder_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let (Some(name), Some(path_str)) = (
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string()),
                        path.to_str().map(|s| s.to_string()),
                    ) {
                        dir_map.insert(path_str, name);
                    }
                }
            }
        }

        let existing_states = self.scroll_area.state();
        let mut import_items: Vec<ImportItem> = Vec::new();

        // Existing icons: keep if file still in the directory
        for state in &existing_states {
            let still_present = state
                .path
                .as_ref()
                .map(|p| dir_map.contains_key(p.as_ref()))
                .unwrap_or(false);
            import_items.push(ImportItem {
                state: IconState {
                    title: state.title.clone(),
                    path: state.path.clone(),
                },
                keep: still_present,
            });
            // Remove matched entry so remaining dir_map entries are new items
            if let Some(ref p) = state.path {
                dir_map.remove(p.as_ref());
            }
        }

        // New items from directory not already in the fence
        for (path_str, name) in dir_map {
            import_items.push(ImportItem {
                state: IconState {
                    title: Arc::from(name.as_str()),
                    path: Some(Arc::from(path_str.as_str())),
                },
                keep: true,
            });
        }

        let fence = self.clone();
        let import_dialog = match ImportDialog::create_window(import_items, move |kept_states| {
            fence
                .scroll_area
                .add_icons_from_state(kept_states.iter(), true);
        }) {
            Ok(import_dialog) => import_dialog,
            Err(e) => {
                error!("{}", e);
                return;
            }
        };
        *App::get().import_dialog.lock() = Some(import_dialog);
    }

    pub async fn show_import_from_dialog(self: &Arc<Self>) {
        if let Some(path_str) = prompt::browse_for_folder().await {
            self.set_imported_from(Some(Arc::from(path_str.as_str())));
            self.show_import_existing_dialog();
        }
    }

    pub async fn from_folder_selector(cover: &DesktopCover) -> Result<Option<Arc<Self>>> {
        let path_str = match prompt::browse_for_folder().await {
            Some(p) => p,
            None => return Ok(None),
        };
        let folder_path = Path::new(&path_str);
        let title = folder_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Folder Fence");

        let bounds = App::get().screen_bounds();
        let width = bounds.width.load(Ordering::Relaxed);
        let height = bounds.height.load(Ordering::Relaxed);
        let fence = Self::new(cover, width / 2 - 150, height / 2 - 75, title)?;
        fence.set_imported_from(Some(Arc::from(path_str.as_str())));

        if let Ok(entries) = std::fs::read_dir(folder_path) {
            fence.scroll_area.add_icons_from_state(
                entries.filter_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    if !path.is_file() {
                        return None;
                    }
                    let name = path.file_stem().and_then(|s| s.to_str())?;
                    let path_str = path.to_str()?;
                    Some(IconState {
                        title: Arc::from(name),
                        path: Some(Arc::from(path_str)),
                    })
                }),
                true,
            );
        }

        Ok(Some(fence))
    }

    pub fn add_area(&self, dl: i32, dt: i32, dw: i32, dh: i32) {
        self.base.add_area(dl, dt, dw, dh);

        let area = self.base.area();
        let fence_area = Area::new(
            area.x.load(Ordering::Relaxed),
            area.y.load(Ordering::Relaxed),
            area.width.load(Ordering::Relaxed),
            area.height.load(Ordering::Relaxed),
        );

        let title_area = TitleBar::area_from_fence_area(&fence_area);
        let scroll_area = ScrollArea::area_from_fence_area(&fence_area);

        unsafe {
            let hdwp = BeginDeferWindowPos(2);
            if hdwp.is_null() {
                panic!("hdwp is null");
            }
            let hdwp = self.title_bar.base().resize_to_deferred(
                title_area.x,
                title_area.y,
                title_area.width,
                title_area.height,
                hdwp,
            );
            let hdwp = self.scroll_area.base().resize_to_deferred(
                scroll_area.x,
                scroll_area.y,
                scroll_area.width,
                scroll_area.height,
                hdwp,
            );
            let _ = EndDeferWindowPos(hdwp);
        }

        self.scroll_area.reflow_icons();
    }

    pub fn paint_with_alpha(&self) {
        // https://stackoverflow.com/a/18613002
        let hwnd = self.base().hwnd();
        unsafe {
            let hdc_screen = GetDC(std::ptr::null_mut());
            let hdc_mem = CreateCompatibleDC(hdc_screen);

            let area = self.base.area();
            let width = area.width.load(Ordering::Relaxed);
            let height = area.height.load(Ordering::Relaxed);

            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width;
            bmi.bmiHeader.biHeight = -height; // top-down
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;

            let mut bits = std::ptr::null_mut();
            let h_bitmap = CreateDIBSection(
                hdc_mem,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            let old_bitmap = SelectObject(hdc_mem, h_bitmap as HGDIOBJ);
            let pixel_count = (width * height) as usize;
            let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, pixel_count);
            let config = App::config();
            for p in pixels.iter_mut() {
                *p = config.fence.fence_bg_color.argb();
            }
            SendMessageW(
                hwnd,
                WM_PRINT,
                hdc_mem as WPARAM,
                (PRF_CLIENT | PRF_CHILDREN | PRF_OWNED) as LPARAM,
            );

            let size = SIZE {
                cx: width,
                cy: height,
            };
            let pt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            let _ = UpdateLayeredWindow(
                hwnd,
                hdc_screen,
                std::ptr::null(),
                &size,
                hdc_mem,
                &pt_src,
                0,
                &blend,
                ULW_ALPHA,
            );

            // TODO: cache h_bitmap, hdc_mem
            SelectObject(hdc_mem, old_bitmap);
            let _ = DeleteObject(h_bitmap.into());
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(std::ptr::null_mut(), hdc_screen);
        }
    }

    pub fn hitman(&self) -> &HitManager {
        &self.hitman
    }

    pub fn unfocus(&self) {
        self.hitman.m.lock().take();
        self.scroll_area.select_icon(None);
    }

    /// Shows the context menu at absolute mouse position x, y
    pub fn show_context_menu(&self, x: i32, y: i32) {
        let hwnd = self.base().hwnd();
        let h_menu = unsafe { CreatePopupMenu() };

        unsafe {
            let _ = AppendMenuW(h_menu, MF_STRING, IDM_IMPORT, w!("&Import"));
            let _ = AppendMenuW(h_menu, MF_STRING, IDM_IMPORT_FROM, w!("Import &from..."));
            let open_explorer_flags = if self.imported_from().is_some() {
                MF_STRING
            } else {
                MF_STRING | MF_GRAYED
            };
            let _ = AppendMenuW(
                h_menu,
                open_explorer_flags,
                IDM_OPEN_EXPLORER,
                w!("Open in Explorer"),
            );
            let _ = AppendMenuW(h_menu, MF_STRING, IDM_ADD_ICON, w!("Add &icon"));
            let _ = AppendMenuW(h_menu, MF_SEPARATOR, 0, std::ptr::null());

            let h_sticky_menu = CreatePopupMenu();
            let checky_sticky = |pos: Option<FenceStickyPosition>| {
                if self.sticky() == pos {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                }
            };

            let _ = AppendMenuW(
                h_sticky_menu,
                MF_STRING | checky_sticky(None),
                IDM_STICKY_NONE,
                w!("None"),
            );
            let _ = AppendMenuW(
                h_sticky_menu,
                MF_STRING | checky_sticky(Some(FenceStickyPosition::TopLeft)),
                IDM_STICKY_TOPLEFT,
                w!("Top Left"),
            );
            let _ = AppendMenuW(
                h_sticky_menu,
                MF_STRING | checky_sticky(Some(FenceStickyPosition::TopRight)),
                IDM_STICKY_TOPRIGHT,
                w!("Top Right"),
            );
            let _ = AppendMenuW(
                h_sticky_menu,
                MF_STRING | checky_sticky(Some(FenceStickyPosition::BottomLeft)),
                IDM_STICKY_BOTTOMLEFT,
                w!("Bottom Left"),
            );
            let _ = AppendMenuW(
                h_sticky_menu,
                MF_STRING | checky_sticky(Some(FenceStickyPosition::BottomRight)),
                IDM_STICKY_BOTTOMRIGHT,
                w!("Bottom Right"),
            );

            let _ = AppendMenuW(
                h_menu,
                MF_POPUP,
                h_sticky_menu as usize,
                w!("Sticky position"),
            );

            let _ = AppendMenuW(h_menu, MF_STRING, IDM_RENAME_FENCE, w!("Re&name fence"));
            let _ = AppendMenuW(h_menu, MF_STRING, IDM_DELETE_FENCE, w!("&Delete fence"));

            let _ = SetForegroundWindow(hwnd);
            let _ = TrackPopupMenu(
                h_menu,
                TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                x,
                y,
                0,
                hwnd,
                std::ptr::null(),
            );
            let _ = DestroyMenu(h_menu);
        }
    }

    fn on_command(self: &Arc<Self>, cover: &DesktopCover, command: usize, hit_type: Hit) -> bool {
        let mut should_save = false;

        match command {
            IDM_ADD_ICON => {
                self.scroll_area.add_icon("", None);
                should_save = true;
            }
            IDM_RENAME_FENCE => {
                let fence = self.clone();
                let current_title = String::from(&fence.title() as &str);
                cover.executor().spawn(async move {
                    if let Some(new_title) =
                        prompt::input("Rename fence", "Enter new fence name:", &current_title).await
                    {
                        if !new_title.is_empty() {
                            fence.set_title(new_title.into());
                            App::get().save_thread.get().unwrap().set_unsaved();
                        }
                    }
                });
            }
            IDM_DELETE_FENCE => {
                let fence = self.clone();
                cover.executor().spawn(async move {
                    let result = prompt::confirm(
                        "Are you sure you want to delete this fence?",
                        "Confirm Deletion",
                        MB_YESNO | MB_ICONQUESTION,
                    )
                    .await;
                    if result == IDYES {
                        let app = App::get();
                        app.fences.lock().remove(&fence);
                        app.save_thread.get().unwrap().set_unsaved();
                    }
                });
            }
            IDM_RUN_ICON => {
                if let Hit::Icon(icon) = hit_type {
                    icon.run();
                }
            }
            IDM_RENAME_ICON => {
                if let Hit::Icon(icon) = hit_type {
                    let current_title = String::from(&icon.title() as &str);
                    cover.executor().spawn(async move {
                        if let Some(new_title) =
                            prompt::input("Rename icon", "Enter new icon name:", &current_title)
                                .await
                        {
                            if !new_title.is_empty() {
                                icon.set_title(new_title.into());
                                App::get().save_thread.get().unwrap().set_unsaved();
                            }
                        }
                    });
                }
            }
            IDM_SET_ICON_PATH => {
                if let Hit::Icon(icon) = hit_type {
                    icon.set_info_from_selector();
                    should_save = true;
                }
            }
            IDM_DELETE_ICON => {
                if let Hit::Icon(icon) = hit_type {
                    let fence = self.clone();
                    cover.executor().spawn(async move {
                        let result = prompt::confirm(
                            "Are you sure you want to delete this icon?",
                            "Confirm Deletion",
                            MB_YESNO | MB_ICONQUESTION,
                        )
                        .await;
                        if result == IDYES {
                            fence.scroll_area.remove_icon(&icon);
                            let app = App::get();
                            app.save_thread.get().unwrap().set_unsaved();
                        }
                    });
                }
            }
            IDM_IMPORT => {
                // Import dialog should be spawned after every event from the queue is processed
                if self.imported_from().is_some() {
                    let fence = self.clone();
                    cover.executor().spawn(async move {
                        fence.show_import_existing_dialog();
                    });
                } else {
                    let fence = self.clone();
                    cover.executor().spawn(async move {
                        fence.show_import_from_dialog().await;
                    });
                }
                should_save = true;
            }
            IDM_IMPORT_FROM => {
                let fence = self.clone();
                cover.executor().spawn(async move {
                    fence.show_import_from_dialog().await;
                });
                should_save = true;
            }
            IDM_STICKY_NONE
            | IDM_STICKY_TOPLEFT
            | IDM_STICKY_TOPRIGHT
            | IDM_STICKY_BOTTOMLEFT
            | IDM_STICKY_BOTTOMRIGHT => {
                let sticky = match command {
                    IDM_STICKY_TOPLEFT => Some(FenceStickyPosition::TopLeft),
                    IDM_STICKY_TOPRIGHT => Some(FenceStickyPosition::TopRight),
                    IDM_STICKY_BOTTOMLEFT => Some(FenceStickyPosition::BottomLeft),
                    IDM_STICKY_BOTTOMRIGHT => Some(FenceStickyPosition::BottomRight),
                    _ => None,
                };
                self.set_sticky(sticky);
                should_save = true;
            }
            IDM_OPEN_EXPLORER => {
                if let Some(import_path) = self.imported_from() {
                    let path_wide: Vec<u16> = import_path
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    unsafe {
                        let _ = ShellExecuteW(
                            std::ptr::null_mut(),
                            w!("open"),
                            path_wide.as_ptr(),
                            std::ptr::null(),
                            std::ptr::null(),
                            SW_SHOWNORMAL,
                        );
                    }
                }
            }
            other => {
                panic!("invalid command: {}", other);
            }
        }

        should_save
    }

    /// Handles WM_DROPFILES: extracts file paths from the drop and adds them as icons.
    fn on_drop_files(&self, hdrop: HDROP, rel_x: i32, rel_y: i32) {
        let config = App::config();
        let title_h = config.fence.title_bar_height;
        let border = config.fence.border_thickness;
        let area = self.base().area();
        let client_width = area.width.load(Ordering::Relaxed) - border * 2;
        let client_height = area.height.load(Ordering::Relaxed) - title_h - border;

        // Only accept drops within the scroll area (below title bar, inside borders)
        if rel_x < border || rel_x >= border + client_width {
            unsafe { DragFinish(hdrop) };
            return;
        }
        if rel_y < title_h || rel_y >= title_h + client_height {
            unsafe { DragFinish(hdrop) };
            return;
        }

        // Collect file paths from the drop
        let file_count = unsafe { DragQueryFileW(hdrop, 0xFFFFFFFF, std::ptr::null_mut(), 0) };
        let mut states: Vec<IconState> = Vec::new();
        let import_dir = self.imported_from();

        for i in 0..file_count {
            let char_count = unsafe { DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0) } as usize;
            let mut buf: Vec<u16> = vec![0u16; char_count + 1];
            unsafe {
                let _ = DragQueryFileW(hdrop, i, buf.as_mut_ptr(), buf.len() as u32);
            };
            let path_str = String::from_utf16_lossy(&buf[..char_count]);
            let src = std::path::Path::new(&path_str);
            if !src.is_file() {
                continue;
            }

            let (name, final_path) = if let Some(ref import_dir) = import_dir {
                // Copy file to import directory
                let dest = std::path::Path::new(import_dir.as_ref())
                    .join(src.file_name().unwrap_or_default());
                if let Err(e) = std::fs::copy(src, &dest) {
                    error!("Failed to copy {} to {}: {}", path_str, dest.display(), e);
                    continue;
                }
                let name = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let final_path = dest.to_str().unwrap_or("");
                (name.to_string(), final_path.to_string())
            } else {
                let name = src
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string());
                let final_path = src.to_str().map(|s| s.to_string());
                match (name, final_path) {
                    (Some(n), Some(p)) => (n, p),
                    _ => continue,
                }
            };

            states.push(IconState {
                title: Arc::from(name.as_str()),
                path: Some(Arc::from(final_path.as_str())),
            });
        }

        unsafe { DragFinish(hdrop) };

        if states.is_empty() {
            return;
        }

        self.scroll_area.add_icons_from_state(states.iter(), false);
        self.scroll_area.base().redraw(true);
        App::get().save_thread.get().unwrap().set_unsaved();
    }
}

impl Window for Fence {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_DISPLAYCHANGE => {
                self.unfocus();
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_SETCURSOR => {
                let mut pt = POINT { x: 0, y: 0 };
                unsafe {
                    let _ = GetCursorPos(&mut pt);
                    let _ = ScreenToClient(hwnd, &mut pt);
                };

                if let Some(cursor) = self.hitman.on_set_cursor(self, pt.x, pt.y) {
                    unsafe {
                        SetCursor(cursor);
                        TRUE as isize
                    }
                } else {
                    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
                }
            }
            WM_LBUTTONDBLCLK => {
                let rel_x = (lparam & 0xFFFF) as i16 as i32;
                let rel_y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                self.hitman.on_lbutton_dblclk(self, rel_x, rel_y);
                0
            }
            WM_LBUTTONDOWN => {
                let rel_x = (lparam & 0xFFFF) as i16 as i32;
                let rel_y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                if self.hitman.on_lbutton_down(self, rel_x, rel_y) {
                    let mut pt = POINT { x: 0, y: 0 };
                    unsafe {
                        let _ = GetCursorPos(&mut pt);
                    };

                    if App::config().use_layered_window {
                        let mut last = self.last_mouse_pos.lock();
                        *last = pt;
                        unsafe {
                            let _ = SetCapture(hwnd);
                        }
                    } else {
                        let cover = App::get().cover.get().unwrap();
                        cover.capture_mouse(Weak::upgrade(&self.self_weak).unwrap(), pt);
                    }
                }
                0
            }
            WM_MOVE if App::config().use_layered_window => unsafe {
                self.scroll_area.base().redraw(false);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            },
            WM_MOUSEMOVE if App::config().use_layered_window => {
                let mut pt = POINT { x: 0, y: 0 };
                unsafe {
                    let _ = GetCursorPos(&mut pt);
                };

                let mut last = self.last_mouse_pos.lock();
                let dx = pt.x - last.x;
                let dy = pt.y - last.y;
                *last = pt;
                drop(last);

                self.hitman.on_mouse_move(self, dx, dy);
                0
            }
            WM_LBUTTONUP if App::config().use_layered_window => {
                let rel_x = (lparam & 0xFFFF) as i16 as i32;
                let rel_y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                self.hitman.on_lbutton_up(self, rel_x, rel_y);
                unsafe {
                    let _ = ReleaseCapture();
                };
                0
            }
            WM_RBUTTONUP => {
                let rel_x = (lparam & 0xFFFF) as i16 as i32;
                let rel_y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                self.hitman.on_rbutton_up(self, rel_x, rel_y);
                0
            }
            WM_PAINT if !App::config().use_layered_window => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect: RECT = std::mem::zeroed();
                let _ = GetClientRect(hwnd, &mut rect);
                App::config()
                    .fence
                    .fence_bg_color
                    .paint_background(hdc, &rect);
                let _ = EndPaint(hwnd, &ps);
                0
            },
            WM_USER_PAINT_WITH_ALPHA if App::config().use_layered_window => {
                self.paint_with_alpha();
                0
            }
            WM_DROPFILES => {
                let hdrop = wparam as HDROP;
                let mut pt = POINT { x: 0, y: 0 };
                unsafe {
                    let _ = DragQueryPoint(hdrop, &mut pt);
                };
                let rel_x = pt.x;
                let rel_y = pt.y;
                self.on_drop_files(hdrop, rel_x, rel_y);
                0
            }
            WM_ACTIVATE => {
                let activation = (wparam & 0xFFFF) as u16 as u32;
                if activation == WA_INACTIVE {
                    self.unfocus();
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_COMMAND => {
                let command = (wparam & 0xFFFF) as u16 as usize;
                if let Some(hit) = self.hitman.m.lock().take() {
                    let cover = App::get().cover.get().unwrap();
                    Weak::upgrade(&self.self_weak)
                        .unwrap()
                        .on_command(cover, command, hit);
                }
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
