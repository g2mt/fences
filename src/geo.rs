use std::sync::atomic::{AtomicI32, Ordering};

use serde::{Deserialize, Serialize};
use windows_sys::Win32::Foundation::RECT;

pub trait Scalar {
    fn load(&self) -> i32;
}

impl Scalar for i32 {
    fn load(&self) -> i32 {
        *self
    }
}

impl Scalar for AtomicI32 {
    fn load(&self) -> i32 {
        self.load(Ordering::Relaxed)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Area<S: Scalar> {
    pub x: S,
    pub y: S,
    pub width: S,
    pub height: S,
}

impl<S: Scalar> Area<S> {
    pub fn new<T: Into<S>>(x: T, y: T, width: T, height: T) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
            width: width.into(),
            height: height.into(),
        }
    }
}

impl Into<Area<AtomicI32>> for Area<i32> {
    fn into(self) -> Area<AtomicI32> {
        Area {
            x: self.x.into(),
            y: self.y.into(),
            width: self.width.into(),
            height: self.height.into(),
        }
    }
}

impl<S: Scalar> Into<RECT> for &Area<S> {
    fn into(self) -> RECT {
        let x = self.x.load();
        let y = self.y.load();
        let w = self.width.load();
        let h = self.height.load();
        RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Bounds<S: Scalar> {
    pub width: S,
    pub height: S,
}
