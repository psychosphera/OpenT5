#![allow(dead_code)]

pub mod gpad;
pub mod keyboard;
pub mod mouse;

use crate::*;

use std::sync::atomic::Ordering;

use lazy_static::lazy_static;

lazy_static! {
    static ref APP_ACTIVE: AtomicBool = AtomicBool::new(false);
}

pub fn activate(app_active: bool) {
    APP_ACTIVE.store(app_active, Ordering::SeqCst);
    if app_active == false {
        mouse::deactivate();
    } else {
        mouse::activate(1);
    }
}

fn startup() {
    mouse::startup();
    gpad::startup();
    dvar::clear_modified("in_mouse").unwrap();
}

fn init() {
    dvar::register_bool(
        "in_mouse",
        true,
        dvar::DvarFlags::ARCHIVE | dvar::DvarFlags::LATCHED,
        Some("Initialize the mouse drivers"),
    )
    .unwrap();
    startup();
}

fn is_foreground_window() -> bool {
    platform::get_platform_vars().active_app
}
