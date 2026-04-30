use serde::{Deserialize, Serialize};

use crate::config::color::Color;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub fence: FenceConfig,
    pub icon: IconConfig,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct FontConfig {
    pub family: String,
    pub size: i32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "Segoe UI".to_string(),
            size: 16,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct FenceConfig {
    pub border_thickness: i32,
    pub title_bar_height: i32,
    pub padding: i32,
    pub spacing: i32,
    pub title_text_color: Color,
    pub title_bar_bg_color: Color,
    pub scroll_area_bg_color: Color<true>,
    /// Alpha is not enabled, because there is a limitation where LWA_COLORKEY doesn't work with full rendering
    ///
    /// See:
    /// - https://stackoverflow.com/questions/12252864/winapi-setlayeredwindowattributes-with-lwa-colorkey-only-sets-pixels-to-either
    /// - https://www.magpcss.org/ceforum/viewtopic.php?f=6&t=13382
    pub fence_bg_color: Color,
}

impl Default for FenceConfig {
    fn default() -> Self {
        FenceConfig {
            border_thickness: 3,
            title_bar_height: 24,
            padding: 10,
            spacing: 10,
            title_text_color: Color(0x00FFFFFF),
            title_bar_bg_color: Color(0x00323232),
            scroll_area_bg_color: Color(0xFF7D7D7D),
            fence_bg_color: Color(0x00323232),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct IconConfig {
    pub size: i32,
    pub selected_bg_color: Color<true>,
    pub unselected_bg_color: Color<true>,
    pub text_color: Color,
    pub icon_size_draw: i32,
}

impl Default for IconConfig {
    fn default() -> Self {
        IconConfig {
            size: 64,
            selected_bg_color: Color(0x00FFAA44),
            unselected_bg_color: Color(0x007D7D7D),
            text_color: Color(0x00FFFFFF),
            icon_size_draw: 32,
        }
    }
}
