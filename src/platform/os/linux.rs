// have to do this to deal with warnings created from x11 constants
#![allow(non_upper_case_globals)]

extern crate alloc;

use cfg_if::cfg_if;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandWindowHandle, XlibDisplayHandle,
    XlibWindowHandle, WaylandDisplayHandle,
};

use crate::{
    platform::WindowHandle,
};

// All uses of unsafe in the following function are just for FFI,
// and all of those functions should be safe as called.
// No reason to comment them individually.
#[allow(
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block
)]
pub fn main() {
    gtk4::init().unwrap();
}

impl WindowHandle {
    pub const fn new(handle: RawWindowHandle) -> Self {
        Self(handle)
    }

    pub const fn get(&self) -> RawWindowHandle {
        self.0
    }

    pub const fn get_wayland(&self) -> Option<WaylandWindowHandle> {
        match self.get() {
            RawWindowHandle::Wayland(handle) => Some(handle),
            _ => None,
        }
    }

    pub const fn get_xlib(&self) -> Option<XlibWindowHandle> {
        match self.get() {
            RawWindowHandle::Xlib(handle) => Some(handle),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MonitorHandle {
    Xlib(XlibDisplayHandle),
    Wayland(WaylandDisplayHandle),
}

#[allow(clippy::missing_trait_methods)]
impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match *self {
            Self::Xlib(handle) => handle
                .display
                .cmp(&other.get_xlib().unwrap().display)
                .then(handle.screen.cmp(&other.get_xlib().unwrap().screen)),
            Self::Wayland(handle) => handle.display.cmp(&other.get_wayland().unwrap().display),
        }
    }
}

#[allow(clippy::missing_trait_methods)]
impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl MonitorHandle {
    cfg_if! {
        if #[cfg(feature = "linux_use_wayland")] {
            pub const fn get(&self) -> RawDisplayHandle {
                match *self {
                    Self::Wayland(handle) => RawDisplayHandle::Wayland(handle),
                    _ => panic!()
                }
            }
        } else {
            pub const fn get(&self) -> RawDisplayHandle {
                match *self {
                    Self::Xlib(handle) => RawDisplayHandle::Xlib(handle),
                    _ => panic!()
                }
            }
        }
    }

    pub const fn get_xlib(&self) -> Option<XlibDisplayHandle> {
        match *self {
            Self::Xlib(handle) => Some(handle),
            _ => None,
        }
    }

    pub const fn get_wayland(&self) -> Option<WaylandDisplayHandle> {
        match *self {
            Self::Wayland(handle) => Some(handle),
            _ => None,
        }
    }
}