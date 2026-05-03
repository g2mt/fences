use serde::{Deserialize, Deserializer, Serialize, Serializer};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;

#[cfg(test)]
mod tests;

/// Represents a color value stored as **AARRGGBB** (alpha, red, green, blue).
///
/// The generic const `ACCEPTS_ALPHA` indicates whether the color variant
/// permits an explicit alpha component. When `true`, the color can be
/// serialized/deserialized from `#AARRGGBB` strings; otherwise only `#RRGGBB`
/// strings are accepted.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color<const ACCEPTS_ALPHA: bool = false>(u32);

#[allow(dead_code)]
impl<const ACCEPTS_ALPHA: bool> Color<ACCEPTS_ALPHA> {
    pub fn from_argb(n: u32) -> Self {
        Self(n)
    }

    pub fn bgr(&self) -> u32 {
        (u32::from(self.b()) << 16) | (u32::from(self.g()) << 8) | u32::from(self.r())
    }

    pub fn abgr(&self) -> u32 {
        (u32::from(self.a()) << 24) | self.bgr()
    }

    pub fn argb(&self) -> u32 {
        self.0
    }

    pub fn r(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    pub fn g(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    pub fn b(&self) -> u8 {
        (self.0 & 0xFF) as u8
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
                        Ok(Color((0xFF << 24) | rgb))
                    }
                    8 => {
                        if !ACCEPTS_ALPHA {
                            return Err(E::custom(
                                "alpha channel not accepted, use #RRGGBB format",
                            ));
                        }
                        let aarrggbb = u32::from_str_radix(hex, 16)
                            .map_err(|_| E::custom("invalid hex digits"))?;
                        Ok(Color(aarrggbb))
                    }
                    _ => Err(E::custom("hex color must be 6 or 8 characters")),
                };
                value
            }
        }

        deserializer.deserialize_str(ColorVisitor::<ACCEPTS_ALPHA>)
    }
}

impl Color<true> {
    pub unsafe fn paint_background(&self, hdc: HDC, rect: &RECT) {
        unsafe {
            let alpha = 0xff - self.a();
            let mem_dc = CreateCompatibleDC(Some(hdc));
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let bitmap = CreateCompatibleBitmap(hdc, width, height);
            SelectObject(mem_dc, bitmap.into());

            let brush = CreateSolidBrush(COLORREF(self.bgr()));
            let local_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            let _ = FillRect(mem_dc, &local_rect, brush);
            let _ = DeleteObject(brush.into());

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: 0,
            };
            let _ = GdiAlphaBlend(hdc, 0, 0, width, height, mem_dc, 0, 0, width, height, blend);

            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(mem_dc);
        }
    }
}

impl Color<false> {
    pub unsafe fn paint_background(&self, hdc: HDC, rect: &RECT) {
        unsafe {
            let brush = CreateSolidBrush(COLORREF(self.abgr()));
            FillRect(hdc, rect, brush);
            let _ = DeleteObject(brush.into());
        }
    }
}
