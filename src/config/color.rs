use serde::{Deserialize, Deserializer, Serialize, Serializer};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

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

impl Color<true> {
    pub unsafe fn paint_background(&self, hdc: HDC, rect: &RECT) {
        let color = self.0;
        let alpha = (color >> 24) as u8;
        if alpha == 255 {
            let brush = CreateSolidBrush(color & 0xFFFFFF);
            FillRect(hdc, rect, brush);
            DeleteObject(brush);
        } else if alpha > 0 {
            let mem_dc = CreateCompatibleDC(hdc);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let bitmap = CreateCompatibleBitmap(hdc, width, height);
            SelectObject(mem_dc, bitmap);

            let brush = CreateSolidBrush(color & 0xFFFFFF);
            let local_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            FillRect(mem_dc, &local_rect, brush);
            DeleteObject(brush);

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: 0,
            };
            GdiAlphaBlend(hdc, 0, 0, width, height, mem_dc, 0, 0, width, height, blend);

            DeleteObject(bitmap);
            DeleteDC(mem_dc);
        }
    }
}
