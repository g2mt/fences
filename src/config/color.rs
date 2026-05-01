use serde::{Deserialize, Deserializer, Serialize, Serializer};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;

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
    /// Red component (0..255).
    pub fn r(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    /// Green component (0..255).
    pub fn g(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    /// Blue component (0..255).
    pub fn b(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Alpha component (0..255).
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
                        // #RRGGBB – parse and store as AARRGGBB with opaque alpha
                        let rgb = u32::from_str_radix(hex, 16)
                            .map_err(|_| E::custom("invalid hex digits"))?;
                        let a = if ACCEPTS_ALPHA { 0xFF } else { 0x0 };
                        // (a << 24) | rgb  gives AARRGGBB, because rgb = 0x00RRGGBB
                        Ok(Color((a << 24) | rgb))
                    }
                    8 => {
                        if !ACCEPTS_ALPHA {
                            return Err(E::custom(
                                "alpha channel not accepted, use #RRGGBB format",
                            ));
                        }
                        // #AARRGGBB – already in AARRGGBB order
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
            let alpha = self.a();
            let color_ref = {
                let r = self.r() as u32;
                let g = self.g() as u32;
                let b = self.b() as u32;
                COLORREF(r | (g << 8) | (b << 16))
            };

            if alpha == 255 {
                let brush = CreateSolidBrush(color_ref);
                FillRect(hdc, rect, brush);
                let _ = DeleteObject(brush.into());
            } else if alpha > 0 {
                let mem_dc = CreateCompatibleDC(Some(hdc));
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;
                let bitmap = CreateCompatibleBitmap(hdc, width, height);
                SelectObject(mem_dc, bitmap.into());

                let brush = CreateSolidBrush(color_ref);
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
}

impl Color<false> {
    pub unsafe fn paint_background(&self, hdc: HDC, rect: &RECT) {
        unsafe {
            let r = self.r() as u32;
            let g = self.g() as u32;
            let b = self.b() as u32;
            let brush = CreateSolidBrush(COLORREF(r | (g << 8) | (b << 16)));
            FillRect(hdc, rect, brush);
            let _ = DeleteObject(brush.into());
        }
    }
}

impl<const ACCEPTS_ALPHA: bool> Into<COLORREF> for Color<ACCEPTS_ALPHA> {
    fn into(self) -> COLORREF {
        let r = self.r() as u32;
        let g = self.g() as u32;
        let b = self.b() as u32;
        COLORREF(r | (g << 8) | (b << 16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_rgb_serialize_deserialize() {
        // Color<false> – #RRGGBB only
        let json = r#""#AABBCC""#;
        let color: Color<false> = serde_json::from_str(json).unwrap();
        assert_eq!(color.r(), 0xAA);
        assert_eq!(color.g(), 0xBB);
        assert_eq!(color.b(), 0xCC);
        assert_eq!(color.a(), 0); // alpha is ignored, stored as 0

        let serialized = serde_json::to_string(&color).unwrap();
        assert_eq!(serialized, json);
    }

    #[test]
    fn test_argb_serialize_deserialize() {
        // Color<true> – #AARRGGBB allowed
        let json = r#""#80AABBCC""#;
        let color: Color<true> = serde_json::from_str(json).unwrap();
        assert_eq!(color.r(), 0xAA);
        assert_eq!(color.g(), 0xBB);
        assert_eq!(color.b(), 0xCC);
        assert_eq!(color.a(), 0x80);

        let serialized = serde_json::to_string(&color).unwrap();
        assert_eq!(serialized, json);
    }

    #[test]
    fn test_argb_roundtrip_with_opaque() {
        // using 6‑digit string
        let json = r#""#336699""#;
        let color: Color<true> = serde_json::from_str(json).unwrap();
        assert_eq!(color.r(), 0x33);
        assert_eq!(color.g(), 0x66);
        assert_eq!(color.b(), 0x99);
        assert_eq!(color.a(), 0xFF); // implicit opaque

        let serialized = serde_json::to_string(&color).unwrap();
        // Should output alpha because ACCEPTS_ALPHA is true
        assert_eq!(serialized, r#""#FF336699""#);
    }

    #[test]
    fn test_rgb_rejects_eight_digits() {
        let json = r#""#80AABBCC""#;
        let result: Result<Color<false>, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_rgb_getters_from_u32() {
        // Internal representation ARGB: A=0xFF, R=0x11, G=0x22, B=0x33
        // So u32 = 0xFF112233
        let color = Color::<false>(0xFF112233);
        assert_eq!(color.r(), 0x11);
        assert_eq!(color.g(), 0x22);
        assert_eq!(color.b(), 0x33);
        assert_eq!(color.a(), 0xFF);
    }

    #[test]
    fn test_argb_into_colorref_conversion() {
        // COLORREF should have R low byte, G middle, B high byte.
        let color = Color::<false>(0xFFAABBCC); // R=0xAA, G=0xBB, B=0xCC
        let cr: COLORREF = color.into();
        let raw: u32 = cr.0;
        // Expected: (0xAA) | (0xBB << 8) | (0xCC << 16)
        assert_eq!(raw, 0x00CCBBAA);
    }

    #[test]
    fn test_roundtrip_with_json_rgb() {
        let json = r#""#F0E1D2""#;
        let color: Color<false> = serde_json::from_str(json).unwrap();
        let back = serde_json::to_string(&color).unwrap();
        assert_eq!(json, back);
    }

    #[test]
    fn test_roundtrip_with_json_argb() {
        let json = r#""#AB12CD34""#;
        let color: Color<true> = serde_json::from_str(json).unwrap();
        let back = serde_json::to_string(&color).unwrap();
        assert_eq!(json, back);
    }
}
