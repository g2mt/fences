use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub fence: FenceConfig,
    pub icon: IconConfig,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FenceConfig {
    pub border_thickness: i32,
    pub title_bar_height: i32,
    pub padding: i32,
    pub spacing: i32,
    pub border_color: u32,
    pub title_bar_bg_color: u32,
    pub title_text_color: u32,
    pub scroll_area_bg_color: u32,
    pub fence_bg_color: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IconConfig {
    pub size: i32,
    pub selected_bg_color: u32,
    pub unselected_bg_color: u32,
    pub text_color: u32,
    pub icon_size_draw: i32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            fence: FenceConfig::default(),
            icon: IconConfig::default(),
        }
    }
}

impl Default for FenceConfig {
    fn default() -> Self {
        FenceConfig {
            border_thickness: 3,
            title_bar_height: 24,
            padding: 10,
            spacing: 10,
            border_color: 0x00323232,
            title_bar_bg_color: 0x00323232,
            title_text_color: 0x00FFFFFF,
            scroll_area_bg_color: 0x007D7D7D,
            fence_bg_color: 0x000000FF,
        }
    }
}

impl Default for IconConfig {
    fn default() -> Self {
        IconConfig {
            size: 64,
            selected_bg_color: 0x00FFAA44,
            unselected_bg_color: 0x007D7D7D,
            text_color: 0x00FFFFFF,
            icon_size_draw: 32,
        }
    }
}
