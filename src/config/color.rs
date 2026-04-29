use serde::{Deserialize, Deserializer, Serialize, Serializer};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::*;

/// Represents a color value stored as **AABBGGRR** (alpha, blue, green, red).
///
/// The generic const `ACCEPTS_ALPHA` indicates whether the color variant
/// permits an explicit alpha component. When `true`, the color can be
/// serialized/deserialized from `#AARRGGBB` strings; otherwise only `#RRGGBB`
/// strings are accepted.
///
/// Internally the value is always stored in the
/// Windows `COLORREF` format with an optional alpha byte in the most‑significant
/// position.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color<const ACCEPTS_ALPHA: bool = false>(pub u32);

#[allow(dead_code)]
impl<const ACCEPTS_ALPHA: bool> Color<ACCEPTS_ALPHA> {
    pub fn r(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    pub fn g(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    pub fn b(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    pub fn a(&self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }
}

impl<const ACCEPTS_ALPHA: bool> Serialize for Color<ACCEPTS_ALPHA> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = if ACCEPTS_ALPHA {
            // Include alpha when allowed
            format!("#{:08X}", self.0)
        } else {
            // Omit alpha component
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
                        // #RRGGBB – parse and convert to AABBGGRR with opaque alpha
                        let rgb = u32::from_str_radix(hex, 16)
                            .map_err(|_| E::custom("invalid hex digits"))?;
                        let r = (rgb >> 16) & 0xFF;
                        let g = (rgb >> 8) & 0xFF;
                        let b = rgb & 0xFF;
                        let a = if ACCEPTS_ALPHA { 0xFF } else { 0x0 };
                        // Alpha = 0xFF (opaque)
                        (a << 24) | (b << 16) | (g << 8) | r
                    }
                    8 => {
                        // #AARRGGBB – parse and reorder to AABBGGRR
                        let argb = u32::from_str_radix(hex, 16)
                            .map_err(|_| E::custom("invalid hex digits"))?;
                        let a = if ACCEPTS_ALPHA {
                            (argb >> 24) & 0xFF
                        } else {
                            0x0
                        };
                        let r = (argb >> 16) & 0xFF;
                        let g = (argb >> 8) & 0xFF;
                        let b = argb & 0xFF;
                        (a << 24) | (b << 16) | (g << 8) | r
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
        unsafe {
            let color = self.0;
            let alpha = self.a();
            if alpha == 255 {
                let brush = CreateSolidBrush(COLORREF(color & 0xFFFFFF));
                FillRect(hdc, rect, brush);
                DeleteObject(brush);
            } else if alpha > 0 {
                let mem_dc = CreateCompatibleDC(hdc);
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;
                let bitmap = CreateCompatibleBitmap(hdc, width, height);
                SelectObject(mem_dc, bitmap);

                let brush = CreateSolidBrush(COLORREF(color & 0xFFFFFF));
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
}

impl Color<false> {
    pub unsafe fn paint_background(&self, hdc: HDC, rect: &RECT) {
        unsafe {
            let brush = CreateSolidBrush(COLORREF(self.0 & 0xFFFFFF));
            FillRect(hdc, rect, brush);
            DeleteObject(brush);
        }
    }
}
