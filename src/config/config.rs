use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize, Clone, Default)]
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
    pub title_bar_bg_color: Color<true>,
    pub title_text_color: Color,
    pub scroll_area_bg_color: Color<true>,
    pub fence_bg_color: Color<true>,
}

impl Default for FenceConfig {
    fn default() -> Self {
        FenceConfig {
            border_thickness: 3,
            title_bar_height: 24,
            padding: 10,
            spacing: 10,
            title_bar_bg_color: Color(0xFF323232),
            title_text_color: Color(0x00FFFFFF),
            scroll_area_bg_color: Color(0xFF7D7D7D),
            fence_bg_color: Color(0x00000000),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IconConfig {
    pub size: i32,
    pub selected_bg_color: Color,
    pub unselected_bg_color: Color,
    pub text_color: Color,
    pub icon_size_draw: i32,
}

impl Default for IconConfig {
    fn default() -> Self {
        IconConfig {
            size: 64,
            selected_bg_color: Color(0xFFFFAA44),
            unselected_bg_color: Color(0xFF7D7D7D),
            text_color: Color(0xFFFFFFFF),
            icon_size_draw: 32,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color<const ACCEPTS_ALPHA: bool = false>(pub u32);

impl<const ACCEPTS_ALPHA: bool> Serialize for Color<ACCEPTS_ALPHA> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = if ACCEPTS_ALPHA {
            format!("#{:08X}", self.0)
        } else {
            format!("#{:06X}", self.0 & 0x00FFFFFF)
        };
        serializer.serialize_str(&s)
    }
}

impl<'de, const ACCEPTS_ALPHA: bool> Deserialize<'de> for Color<ACCEPTS_ALPHA> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColorVisitor<const ACCEPTS_ALPHA: bool>;

        impl<'de, const ACCEPTS_ALPHA: bool> serde::de::Visitor<'de> for ColorVisitor<ACCEPTS_ALPHA> {
            type Value = Color<ACCEPTS_ALPHA>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                if ACCEPTS_ALPHA {
                    formatter.write_str("a hex color string like \"#RRGGBB\" or \"#AARRGGBB\"")
                } else {
                    formatter.write_str("a hex color string like \"#RRGGBB\"")
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Color<ACCEPTS_ALPHA>, E>
            where
                E: serde::de::Error,
            {
                let hex = v
                    .strip_prefix('#')
                    .ok_or_else(|| E::custom("missing leading #"))?;
                let value = match hex.len() {
                    6 => {
                        let rgb = u32::from_str_radix(hex, 16)
                            .map_err(|_| E::custom("invalid hex digits"))?;
                        if ACCEPTS_ALPHA {
                            (0xFF << 24) | rgb
                        } else {
                            rgb
                        }
                    }
                    8 => {
                        let parsed = u32::from_str_radix(hex, 16)
                            .map_err(|_| E::custom("invalid hex digits"))?;
                        if ACCEPTS_ALPHA {
                            parsed
                        } else {
                            parsed & 0xFFFFFF
                        }
                    }
                    _ => return Err(E::custom("hex color must be 6 or 8 characters")),
                };
                Ok(Color(value))
            }
        }

        deserializer.deserialize_str(ColorVisitor::<ACCEPTS_ALPHA>)
    }
}
