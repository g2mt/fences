use serde::{Deserialize, Serialize, Serializer, Deserializer};

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
    pub border_color: Color,
    pub title_bar_bg_color: Color,
    pub title_text_color: Color,
    pub scroll_area_bg_color: Color,
    pub fence_bg_color: Color,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IconConfig {
    pub size: i32,
    pub selected_bg_color: Color,
    pub unselected_bg_color: Color,
    pub text_color: Color,
    pub icon_size_draw: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color(pub u32);

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("#{:06X}", self.0 & 0xFFFFFF);
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColorVisitor;

        impl<'de> serde::de::Visitor<'de> for ColorVisitor {
            type Value = Color;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a hex color string like \"#RRGGBB\"")
            }

            fn visit_str<E>(self, v: &str) -> Result<Color, E>
            where
                E: serde::de::Error,
            {
                let hex = v.strip_prefix('#').ok_or_else(|| E::custom("missing leading #"))?;
                if hex.len() != 6 {
                    return Err(E::custom("hex color must be 6 characters"));
                }
                let value = u32::from_str_radix(hex, 16).map_err(|_| E::custom("invalid hex digits"))?;
                Ok(Color(value))
            }
        }

        deserializer.deserialize_str(ColorVisitor)
    }
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
            border_color: Color(0x00323232),
            title_bar_bg_color: Color(0x00323232),
            title_text_color: Color(0x00FFFFFF),
            scroll_area_bg_color: Color(0x007D7D7D),
            fence_bg_color: Color(0x000000FF),
        }
    }
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
