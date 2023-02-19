#![allow(dead_code)]

use crate::{util::*, *, platform::WindowHandle};
use pollster::block_on;
use raw_window_handle::HasRawWindowHandle;
use sscanf::scanf;
use winit::dpi::Position;
use std::collections::{HashSet, VecDeque};
use std::sync::RwLock;

use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, WindowBuilder},
};

pub const MIN_HORIZONTAL_RESOLUTION: u16 = 640;
pub const MIN_VERTICAL_RESOLUTION: u16 = 480;

fn init_render_thread() {
    if !sys::spawn_render_thread(rb::render_thread) {
        com::errorln(com::ErrorParm::FATAL, "Failed to create render thread");
    }
}

pub fn init_threads() {
    com::println(8.into(), &format!(
        "{}: Trying SMP acceleration...",
        std::thread::current().name().unwrap_or("main")
    ));
    init_render_thread();
    //init_worker_threads();
    com::println(8.into(), &format!(
        "{}: ...succeeded",
        std::thread::current().name().unwrap_or("main")
    ));
}

pub fn begin_registration_internal() -> Result<(), ()> {
    com::println(8.into(), &format!(
        "{}: render::begin_registration_internal()...",
        std::thread::current().name().unwrap_or("main")
    ));

    if init().is_err() {
        return Err(());
    }
    sys::wait_event("rg_registered", usize::MAX);
    Ok(())
}

fn register() {
    register_dvars();
}

fn reflection_probe_register_dvars() {
    dvar::register_bool("r_reflectionProbeGenerate", false, dvar::DvarFlags::empty(), "Generate cube maps for reflection probes.".into()).unwrap();
}

const ASPECT_RATIO_AUTO: &str = "auto";
const ASPECT_RATIO_STANDARD: &str = "standard";
const ASPECT_RATIO_16_10: &str = "wide 16:10";
const ASPECT_RATIO_16_9: &str = "wide 16:9";

fn register_dvars() {
    dvar::register_bool(
        "r_fullscreen",
        true,
        dvar::DvarFlags::ARCHIVE | dvar::DvarFlags::LATCHED,
        Some("Display game full screen"),
    )
    .unwrap();
    dvar::register_enumeration(
        "r_aspectRatio", 
        "auto".into(), 
        Some(vec![
            ASPECT_RATIO_AUTO.into(), 
            ASPECT_RATIO_STANDARD.into(), 
            ASPECT_RATIO_16_10.into(), 
            ASPECT_RATIO_16_9.into()
            ]), 
        dvar::DvarFlags::ARCHIVE | dvar::DvarFlags::LATCHED, 
        Some("Screen aspect ratio.  Most widescreen monitors are 16:10 instead of 16:9.")
    ).unwrap();
    dvar::register_int(
        "r_aaSamples",
        1,
        Some(1),
        Some(16),
        dvar::DvarFlags::ARCHIVE | dvar::DvarFlags::LATCHED,
        Some("Anti-aliasing sample count; 1 disables anti-aliasing"),
    )
    .unwrap();
    dvar::register_bool(
        "r_vsync", 
        true, 
        dvar::DvarFlags::ARCHIVE | dvar::DvarFlags::LATCHED, 
        Some("Enable v-sync before drawing the next frame to avoid \'tearing\' artifacts.")
    ).unwrap();
    dvar::register_string(
        "r_customMode",
        "",
        dvar::DvarFlags::ARCHIVE | dvar::DvarFlags::LATCHED,
        Some("Special resolution mode for the remote debugger"),
    )
    .unwrap();
    dvar::register_int(
        "vid_xpos",
        3,
        Some(-4096),
        4096.into(),
        dvar::DvarFlags::ARCHIVE,
        "Game window horizontal position".into(),
    )
    .unwrap();
    dvar::register_int(
        "vid_ypos",
        3,
        Some(-4096),
        4096.into(),
        dvar::DvarFlags::ARCHIVE,
        "game window vertical position".into(),
    )
    .unwrap();

    reflection_probe_register_dvars();
}

fn init() -> Result<(), ()> {
    com::println(8.into(), &format!(
        "{}: render::init()...",
        std::thread::current().name().unwrap_or("main")
    ));

    register();

    env_logger::init();
    init_graphics_api().unwrap();

    Ok(())
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum WinitCustomEvent {
    CreateConsole,
    DestroyConsole,
}

#[derive(Default)]
struct WinitGlobals {
    current_monitor_handle: Option<winit::monitor::MonitorHandle>,
    best_monitor_handle: Option<winit::monitor::MonitorHandle>,
    video_modes: Vec<winit::monitor::VideoMode>,
    window_handle: Option<WindowHandle>,
    proxy_events: VecDeque<WinitCustomEvent>
}

lazy_static! {
    static ref WINIT_GLOBALS: Arc<RwLock<WinitGlobals>> =
        Arc::new(RwLock::new(WinitGlobals {
            current_monitor_handle: None,
            best_monitor_handle: None,
            window_handle: None,
            video_modes: Vec::new(),
            proxy_events: VecDeque::new(),
        }));
}

pub fn main_window_handle() -> Option<WindowHandle> {
    let lock = WINIT_GLOBALS.clone();
    let wg = lock.read().unwrap();
    wg.window_handle
}

pub struct RenderGlobals {
    adapter_native_width: u16,
    adapter_native_height: u16,
    adapter_fullscreen_width: u16,
    adapter_fullscreeen_height: u16,
    resolution_names: HashSet<String>,
    refresh_rate_names: HashSet<String>,
    target_window_index: i32,
    window: gfx::WindowTarget,
    device: Option<sys::gpu::Device>,
    adapter: Option<sys::gpu::Adapter>,
    instance: Option<sys::gpu::Instance>,
}

impl RenderGlobals {
    pub fn new() -> Self {
        Self {
            adapter_native_width: MIN_HORIZONTAL_RESOLUTION,
            adapter_native_height: MIN_VERTICAL_RESOLUTION,
            adapter_fullscreen_width: MIN_HORIZONTAL_RESOLUTION,
            adapter_fullscreeen_height: MIN_VERTICAL_RESOLUTION,
            resolution_names: HashSet::new(),
            refresh_rate_names: HashSet::new(),
            target_window_index: 0,
            window: gfx::WindowTarget::new(),
            device: None,
            adapter: None,
            instance: None,
        }
    }
}

impl Default for RenderGlobals {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static! {
    pub static ref RENDER_GLOBALS: Arc<RwLock<RenderGlobals>> =
        Arc::new(RwLock::new(Default::default()));
}

fn fatal_init_error(error: &str) -> ! {
    com::println(8.into(), "********** Device returned an unrecoverable error code during initialization  **********");
    com::println(8.into(), "********** Initialization also happens while playing if Renderer loses a device **********");
    com::println(8.into(), error);
    sys::render_fatal_error();
}

fn set_custom_resolution(wnd_parms: &mut gfx::WindowParms) -> Result<(), ()> {
    dvar::set_string(
        "r_customMode",
        &format!("{}x{}", wnd_parms.display_width, wnd_parms.display_height),
    )
    /*
    match wnd_parms.display_width <= width && wnd_parms.display_height <= height
    {
        true => Ok(()),
        false => Err(()),
    }
    */
}

/*
fn get_video_modes() -> Vec<winit::monitor::VideoMode> {
    if WINIT_GLOBALS
        .clone()
        .try_read()
        .expect("")
        .video_modes
        .is_empty()
    {
        let current_monitor_handle = match WINIT_GLOBALS
            .clone()
            .try_read()
            .expect("")
            .current_monitor_handle
            .clone()
        {
            Some(h) => h,
            None => return Vec::new(),
        };

        return current_monitor_handle.video_modes().collect();
    }

    WINIT_GLOBALS
        .clone()
        .try_read()
        .expect("")
        .video_modes
        .clone()
}
*/

fn closest_refresh_rate_for_mode(
    width: u16,
    height: u16,
    hz: u16,
) -> Option<u16> {
    let video_modes = WINIT_GLOBALS.clone().read().unwrap().video_modes.clone();
    if video_modes.is_empty() {
        return Some(60);
    }
    let mode = video_modes.iter().find(|&m| {
        ((m.refresh_rate_millihertz() - (m.refresh_rate_millihertz() % 1000))
            / 1000
            == hz as _)
            && m.size().width == width as _
            && m.size().height == height as _
    });

    if let Some(..) = mode {
        return Some((mode.unwrap().refresh_rate_millihertz() / 1000) as _);
    }

    let mode = video_modes
        .iter()
        .find(|&m| (m.refresh_rate_millihertz() / 1000) == hz as _);
    if let Some(..) = mode {
        return Some((mode.unwrap().refresh_rate_millihertz() / 1000) as _);
    }

    let mode = video_modes.iter().find(|&m| {
        m.size().width == width as _ && m.size().height == height as _
    });

    if let Some(..) = mode {
        return Some((mode.unwrap().refresh_rate_millihertz() / 1000) as _);
    }

    None
}

fn set_wnd_parms(wnd_parms: &mut gfx::WindowParms) {
    let r_fullscreen = dvar::get_bool("r_fullscreen").unwrap();
    wnd_parms.fullscreen = r_fullscreen;
    /*
    if !r_fullscreen {
        if set_custom_resolution(wnd_parms).is_err() {
            let r_mode = dvar::get_enumeration("r_mode").unwrap();
            (wnd_parms.display_width, wnd_parms.display_height) = scanf!(r_mode, "{}x{}", u16, u16).unwrap();
        }
    }
    */

    let r_mode = dvar::get_enumeration("r_mode").unwrap();
    (wnd_parms.display_width, wnd_parms.display_height) =
        scanf!(r_mode, "{}x{}", u16, u16).unwrap();

    if !wnd_parms.fullscreen {
        let lock = RENDER_GLOBALS.clone();
        let render_globals = lock.read().expect("");

        if render_globals.adapter_native_width < wnd_parms.display_width {
            wnd_parms.display_width = wnd_parms
                .display_width
                .clamp(0, render_globals.adapter_native_width);
        }
        if render_globals.adapter_native_height < wnd_parms.display_height {
            wnd_parms.display_height = wnd_parms
                .display_height
                .clamp(0, render_globals.adapter_native_height);
        }
    }

    wnd_parms.scene_width = wnd_parms.display_width;
    wnd_parms.scene_height = wnd_parms.display_height;

    if !wnd_parms.fullscreen {
        wnd_parms.hz = 60;
    } else {
        let hz = closest_refresh_rate_for_mode(
            wnd_parms.display_width,
            wnd_parms.display_height,
            wnd_parms.hz,
        )
        .unwrap();
        wnd_parms.hz = hz;
        dvar::set_string_internal("r_displayRefresh", &format!("{} Hz", hz))
            .unwrap();
    }

    wnd_parms.x = dvar::get_int("vid_xpos").unwrap() as _;
    wnd_parms.y = dvar::get_int("vid_ypos").unwrap() as _;
    wnd_parms.aa_samples = dvar::get_int("r_aaSamples").unwrap() as _;
}

fn store_window_settings(wnd_parms: &mut gfx::WindowParms) -> Result<(), ()> {
    let lock = vid::CONFIG.clone();
    let mut vid_config = lock.write().unwrap();

    vid_config.scene_width = wnd_parms.scene_width;
    vid_config.scene_height = wnd_parms.scene_height;
    vid_config.display_width = wnd_parms.display_width;
    vid_config.display_height = wnd_parms.display_height;
    vid_config.display_frequency = wnd_parms.hz;
    vid_config.is_fullscreen = wnd_parms.fullscreen;

    vid_config.aspect_ratio_window =
        match dvar::get_enumeration("r_aspectRatio").unwrap().as_str() {
            ASPECT_RATIO_AUTO => {
                let render_globals_lock = RENDER_GLOBALS.clone();
                let render_globals = render_globals_lock.write().unwrap();

                let (display_width, display_height) =
                    if vid_config.is_fullscreen {
                        (
                            render_globals.adapter_native_width as f32,
                            render_globals.adapter_native_height as f32,
                        )
                    } else {
                        (
                            vid_config.display_width as f32,
                            vid_config.display_height as f32,
                        )
                    };

                if display_width / display_height == 16.0 / 10.0 {
                    16.0 / 10.0
                } else if display_width / display_height > 16.0 / 10.0 {
                    16.0 / 9.0
                } else {
                    4.0 / 3.0
                }
            }
            ASPECT_RATIO_STANDARD => 4.0 / 3.0,
            ASPECT_RATIO_16_10 => 16.0 / 10.0,
            ASPECT_RATIO_16_9 => 16.0 / 9.0,
            _ => panic!(
                "unhandled case, aspectRatio = {}",
                dvar::get_enumeration("r_aspectRatio").unwrap()
            ),
        };

    dvar::set_bool_internal(
        "wideScreen",
        vid_config.aspect_ratio_window != 4.0 / 3.0,
    )
    .unwrap();
    vid_config.aspect_ratio_scene_pixel = (vid_config.scene_height as f32
        * vid_config.aspect_ratio_window)
        / vid_config.scene_width as f32;

    let render_globals_lock = RENDER_GLOBALS.clone();
    let render_globals = render_globals_lock.write().unwrap();

    vid_config.aspect_ratio_display_pixel = if !vid_config.is_fullscreen {
        1.0
    } else {
        (render_globals.adapter_fullscreeen_height as f32
            * vid_config.aspect_ratio_window)
            / render_globals.adapter_fullscreen_width as f32
    };

    vid_config.is_tool_mode =
        if let Some(enabled) = dvar::get_bool("r_reflectionProbeGenerate") {
            enabled
        } else {
            false
        };

    Ok(())
}

#[allow(clippy::collapsible_else_if)]
fn reduce_window_settings() -> Result<(), ()> {
    if dvar::get_int("r_aaSamples").unwrap() > 1 {
        dvar::set_int("r_aaSamples", dvar::get_int("r_aaSamples").unwrap() - 1)
    } else {
        if dvar::get_enumeration("r_displayRefresh")
            .unwrap()
            .is_empty()
            || vid::config().display_frequency < 60
        {
            if dvar::get_enumeration("r_mode").unwrap().is_empty()
                || vid::config().display_width < MIN_HORIZONTAL_RESOLUTION
                || vid::config().display_height < MIN_VERTICAL_RESOLUTION
            {
                Err(())
            } else {
                dvar::set_enumeration_prev("r_mode")
            }
        } else {
            dvar::set_enumeration_prev("r_displayRefresh")
        }
    }
}

fn choose_adapter() -> Option<sys::gpu::Adapter> {
    let instance = sys::gpu::Instance::new();
    Some(block_on(sys::gpu::Adapter::new(&instance, None)))
}

fn pre_create_window() -> Result<(), ()> {
    com::println(8.into(), "Getting Device interface...");
    let instance = sys::gpu::Instance::new();
    let adapter = block_on(sys::gpu::Adapter::new(&instance, None));
    RENDER_GLOBALS.clone().write().expect("").device =
        match block_on(sys::gpu::Device::new(&adapter)) {
            Some(d) => Some(d),
            None => {
                com::println(8.into(), "Device failed to initialize.");
                return Err(());
            }
        };

    RENDER_GLOBALS.clone().write().expect("").adapter = choose_adapter();
    dvar::register_enumeration(
        "r_mode",
        "640x480".into(),
        Some(vec!["640x480".into()]),
        dvar::DvarFlags::ARCHIVE | dvar::DvarFlags::LATCHED,
        Some("Renderer resolution mode"),
    )
    .unwrap();

    dvar::register_enumeration(
        "r_displayRefresh",
        "60 Hz".into(),
        Some(vec!["60 Hz".into()]),
        dvar::DvarFlags::ARCHIVE
            | dvar::DvarFlags::LATCHED
            | dvar::DvarFlags::CHANGEABLE_RESET,
        Some("Refresh rate"),
    )
    .unwrap();

    Ok(())
}

lazy_static! {
    pub static ref WINDOW_AWAITING_INIT: Arc<RwLock<SmpEvent<()>>> =
        Arc::new(RwLock::new(SmpEvent::new((), false, false)));
    pub static ref WINDOW_INITIALIZING: Arc<RwLock<SmpEvent<()>>> =
        Arc::new(RwLock::new(SmpEvent::new((), false, false)));
    pub static ref WINDOW_INITIALIZED: Arc<RwLock<SmpEvent<bool>>> =
        Arc::new(RwLock::new(SmpEvent::new(false, false, false)));
}

pub fn create_window_2(wnd_parms: &mut gfx::WindowParms) -> Result<(), ()> {
    {
        let lock = WINDOW_AWAITING_INIT.clone();
        let mut writer = lock.write().unwrap();
        writer.send_cleared(()).unwrap();
    }
    {
        let lock = WINDOW_INITIALIZING.clone();
        let mut writer = lock.write().unwrap();
        writer.send(()).unwrap();
    }

    if wnd_parms.fullscreen {
        com::println(8.into(), &format!(
            "Attempting {} x {} fullscreen with 32 bpp at {} hz",
            wnd_parms.display_width, wnd_parms.display_height, wnd_parms.hz
        ));
    } else {
        com::println(8.into(), &format!(
            "Attempting {} x {} window at ({}, {})",
            wnd_parms.display_width,
            wnd_parms.display_height,
            wnd_parms.x,
            wnd_parms.y
        ));
    }

    let window_name = com::get_official_build_name_r();

    // ========================================================================
    // The following code is done in the original engine's WM_CREATE handler,
    // but winit has no equivalent message for WM_CREATE. Do them here after
    // the window has been created instead

    //platform::set_window_handle(
    //    platform::WindowHandle::new(window.raw_window_handle()));

    // ========================================================================

    let mut modifiers = winit::event::ModifiersState::empty();
    let fullscreen = wnd_parms.fullscreen;
    let width = wnd_parms.scene_width;
    let height = wnd_parms.scene_height;
    let x = wnd_parms.x;
    let y = wnd_parms.y;

    let event_loop = EventLoop::new();
    let main_window = match WindowBuilder::new()
        .with_title(window_name)
        .with_position(PhysicalPosition::<i32>::new(x as _, y as _))
        .with_inner_size(PhysicalSize::new(width, height))
        .with_resizable(true)
        .with_visible(false)
        .with_decorations(!fullscreen)
        .with_window_icon(com::get_icon_rgba())
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            com::println(8.into(), "couldn't create a window.");
            println!("{}", e);
            {
                let lock = WINDOW_INITIALIZING.clone();
                let mut writer = lock.write().unwrap();
                writer.send_cleared(()).unwrap();
            }
            {
                let lock = WINDOW_INITIALIZED.clone();
                let mut writer = lock.write().unwrap();
                writer.send(false).unwrap();
            }
            return Err(());
        }
    };

    main_window.set_visible(true);

    if fullscreen == false {
        main_window.focus_window();
    }

    {
        let lock = WINIT_GLOBALS.clone();
        let mut wg = lock.write().unwrap();
        wg.window_handle = Some(main_window.window_handle());
    }

    com::println(8.into(), "Game window successfully created.");

    // ========================================================================
    // This part is supposed to be done in sys::create_console, but you can't
    // bind windows to an event loop after calling event_loop::run, so instead
    // we create them here, set them to invisible, and then set them to visible
    // in sys::create_console instead of creating them there.
    //
    // I'm not entirely sure how we're going to implement the console for
    // other platforms, so this logic might end up being handled with, e.g.,
    // GTK, instead, but for now we're just going to keep things simple. If
    // we have to move things around later, we can.

    let console_title = com::get_build_display_name();
    let monitor = main_window.current_monitor().or(main_window.available_monitors().nth(0)).unwrap();
    let horzres = (monitor.size().width - 450) / 2;
    let vertres = (monitor.size().height - 600) / 2;
    let s_wcd_lock = conbuf::S_WCD.clone();
    let mut s_wcd = s_wcd_lock.write().unwrap();
    let console_width = s_wcd.window_width;
    let console_height = s_wcd.window_height;
    let console_window = winit::window::WindowBuilder::new()
        .with_title(console_title)
        .with_position(Position::Physical(PhysicalPosition::new(horzres as _, vertres as _)))
        .with_inner_size(PhysicalSize::new(console_width, console_height))
        .build(&event_loop)
        .unwrap();

    s_wcd.window = Some(console_window);
    
    const CODLOGO_POS_X: i32 = 5;
    const CODLOGO_POS_Y: i32 = 5;
    const INPUT_LINE_POS_X: i32 = 6;
    const INPUT_LINE_POS_Y: i32 = 400;
    const INPUT_LINE_SIZE_W: i32 = 608;
    const INPUT_LINE_SIZE_H: i32 = 20;
    const BUFFER_POS_X: i32 = 6;
    const BUFFER_POS_Y: i32 = 70;
    const BUFFER_SIZE_W: i32 = 606;
    const BUFFER_SIZE_H: i32 = 324;

    let parent = Some(s_wcd.window.as_mut().unwrap().raw_window_handle());
    let (cod_logo_window, input_line_window, buffer_window) = unsafe {
        let cod_logo_window = winit::window::WindowBuilder::new()
            .with_parent_window(parent)
            .with_position(PhysicalPosition::new(CODLOGO_POS_X, CODLOGO_POS_Y))
            .with_decorations(false)
            .with_visible(false)
            .build(&event_loop)
            .unwrap();

        let input_line_window = winit::window::WindowBuilder::new()
            .with_parent_window(parent)
            .with_position(PhysicalPosition::new(INPUT_LINE_POS_X, INPUT_LINE_POS_Y))
            .with_inner_size(PhysicalSize::new(INPUT_LINE_SIZE_H, INPUT_LINE_SIZE_W))
            .with_visible(false)
            .build(&event_loop)
            .unwrap();

        let buffer_window = winit::window::WindowBuilder::new()
            .with_parent_window(parent)
            .with_position(PhysicalPosition::new(BUFFER_POS_X, BUFFER_POS_Y))
            .with_inner_size(PhysicalSize::new(BUFFER_SIZE_H, BUFFER_SIZE_W))
            .with_visible(false)
            .build(&event_loop)
            .unwrap();

        (cod_logo_window, input_line_window, buffer_window)
    };

    conbuf::s_wcd_set_cod_logo_window(cod_logo_window);
    conbuf::s_wcd_set_input_line_window(input_line_window);
    conbuf::s_wcd_set_buffer_window(buffer_window);
    // ========================================================================

    event_loop.run(move |event, _, control_flow| match event {
        Event::NewEvents(StartCause::Init) => {
            let monitor = main_window.current_monitor().or(main_window.available_monitors().nth(0)).unwrap();
            let mut modes: Vec<winit::monitor::VideoMode> = monitor.video_modes().collect();
            modes.sort_by(|a, b| a.size().width.cmp(&b.size().width));
            let mut valid_modes: Vec<&winit::monitor::VideoMode> = modes
                .iter()
                .filter(|&m| {
                    m.size().width > MIN_HORIZONTAL_RESOLUTION as _
                    && m.size().height > MIN_VERTICAL_RESOLUTION as _
                })
            .collect();

            valid_modes.sort_by_key(|m| m.size().width);
            valid_modes.sort_by_key(|m| m.refresh_rate_millihertz());

            valid_modes.iter().for_each(|&m| {
                RENDER_GLOBALS
                    .clone()
                    .write()
                    .unwrap()
                    .resolution_names
                    .insert(format!("{}x{}", m.size().width, m.size().height));
                }
            );

            WINIT_GLOBALS.clone().write().unwrap().video_modes = valid_modes.iter().cloned().cloned().collect();
            let width = monitor.size().width;
            let height = monitor.size().height;
            {
               let lock = RENDER_GLOBALS.clone();
               let mut render_globals = lock.write().unwrap();
               render_globals.adapter_native_width = width as _;
               render_globals.adapter_native_height = height as _;
               render_globals.adapter_fullscreen_width = width as _;
               render_globals.adapter_fullscreeen_height = height as _;
            }

            let mode = {
                let lock = RENDER_GLOBALS.clone();
                let render_globals = lock.read().unwrap();
                let mut names: Vec<_> = render_globals.resolution_names.iter().cloned().collect();
                names.sort_by_key(|n| scanf!(n, "{}x{}", u16, u16).unwrap().0);
                names
                .iter()
                .last()
                .unwrap()
                .clone()
            };

            dvar::register_enumeration(
                "r_mode",
                mode,
                Some(Vec::from_iter(
                    RENDER_GLOBALS
                        .clone()
                        .read()
                        .unwrap()
                        .resolution_names
                        .iter()
                        .cloned(),
                )),
                dvar::DvarFlags::ARCHIVE | dvar::DvarFlags::LATCHED,
        Some("Renderer resolution mode"),
            ).unwrap();

            /*
            modes.sort_by(|a, b| {
                a.refresh_rate_millihertz()
                    .cmp(&b.refresh_rate_millihertz())
            });
            */

            modes.iter().for_each(|m| {
                RENDER_GLOBALS
                    .clone()
                    .write()
                    .unwrap()
                    .refresh_rate_names
                    .insert(format!(
                        "{} Hz",
                        (m.refresh_rate_millihertz()
                            - (m.refresh_rate_millihertz() % 1000))
                            / 1000
                    ));
            });

            let refresh = {
                let lock = RENDER_GLOBALS.clone();
                let render_globals = lock.read().unwrap();
                let mut names: Vec<_> = render_globals.refresh_rate_names.iter().cloned().collect();
                names.sort_by_key(|n| scanf!(n, "{} Hz", u16).unwrap());
                names
                .iter()
                .last()
                .unwrap()
                .clone()
            };

            dvar::register_enumeration(
                "r_displayRefresh",
                refresh,
                Some(Vec::from_iter(
                    RENDER_GLOBALS
                        .clone()
                        .read()
                        .unwrap()
                        .refresh_rate_names
                        .iter()
                        .cloned()
                        .collect::<Vec<String>>(),
                    )
                ),
                dvar::DvarFlags::ARCHIVE
                    | dvar::DvarFlags::LATCHED
                    | dvar::DvarFlags::CHANGEABLE_RESET,
                Some("Refresh rate"),
            ).unwrap();

            let mut wnd_parms = gfx::WindowParms::new();
            set_wnd_parms(&mut wnd_parms);
            let width = wnd_parms.display_width;
            let height = wnd_parms.display_height;
            let hz = wnd_parms.hz;

            let window_fullscreen = if fullscreen {
                let modes = main_window.current_monitor().unwrap().video_modes();
                {
                    let lock = WINIT_GLOBALS.clone();
                    let mut winit_globals = lock.write().unwrap();
                    winit_globals.video_modes = modes.collect();
                }
                let modes = main_window.current_monitor().unwrap().video_modes();
                modes.for_each(|v| println!("{}", v));
                let mut modes = main_window.current_monitor().unwrap().video_modes();
                let mode = modes
                    .find(|m| {
                        m.size().width == width as _
                            && m.size().height == height as _
                            && m.refresh_rate_millihertz().div_floor(1000)
                                == hz as _
                    })
                    .unwrap();
                Some(Fullscreen::Exclusive(mode))
            } else {
                None
            };

            main_window.set_fullscreen(window_fullscreen);
            if dvar::get_bool("r_reflectionProbeGenerate").unwrap()
                && dvar::get_bool("r_fullscreen").unwrap() 
            {
                dvar::set_bool_internal("r_fullscreen", false).unwrap();
                cbuf::add_textln(0, "vid_restart");
            }
            dvar::register_bool("r_autopriority",
                false,
                dvar::DvarFlags::ARCHIVE,
                Some("Automatically set the priority of the windows process when the game is minimized"),
            ).unwrap();

            {
                let lock = WINDOW_INITIALIZING.clone();
                let mut writer = lock.write().unwrap();
                writer.send_cleared(()).unwrap();
            }
            {
                let lock = WINDOW_INITIALIZED.clone();
                let mut writer = lock.write().unwrap();
                writer.send(true).unwrap();
            }
        },
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == main_window.id() => match event {
            WindowEvent::Destroyed => {
                //FUN_004dfd60()
                platform::clear_window_handle();
            },
            WindowEvent::ModifiersChanged(m) => {
                modifiers = *m;
            }
            WindowEvent::Moved(position) => {
                if dvar::get_bool("r_fullscreen").unwrap() {
                    input::mouse::activate(0);
                } else {
                    dvar::set_int_internal("vid_xpos", position.x).unwrap();
                    dvar::set_int_internal("vid_ypos", position.y).unwrap();
                    dvar::clear_modified("vid_xpos").unwrap();
                    dvar::clear_modified("vid_ypos").unwrap();
                    if platform::get_platform_vars().active_app {
                        input::activate(true);
                    }
                }
            },
            WindowEvent::Focused(b) => {
                vid::app_activate(*b, platform::get_minimized());
            },
            WindowEvent::CloseRequested => {
                cbuf::add_textln(0, "quit");
                *control_flow = ControlFlow::Exit;
            },
            WindowEvent::MouseWheel {
                delta,
                ..
            } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(f, _) => *f,
                    MouseScrollDelta::PixelDelta(_) =>
                        panic!("render::create_window: unable to handle PixelDelta variant of MouseScrollDelta for MouseWheel event")
                };
                if lines < 0.0 {
                    sys::enqueue_event(
                        sys::Event::new(Some(platform::get_msg_time()),
                        sys::EventType::Mouse(
                            input::mouse::Scancode::MWheelDown,
                            true
                        ),
                        None));
                    sys::enqueue_event(
                        sys::Event::new(Some(platform::get_msg_time()),
                        sys::EventType::Mouse(
                            input::mouse::Scancode::MWheelDown,
                            false),
                        None));
                } else {
                    sys::enqueue_event(
                        sys::Event::new(Some(platform::get_msg_time()),
                        sys::EventType::Mouse(
                            input::mouse::Scancode::MWheelUp,
                            true),
                        None));
                    sys::enqueue_event(
                        sys::Event::new(Some(platform::get_msg_time()),
                        sys::EventType::Mouse(
                            input::mouse::Scancode::MWheelUp,
                            false),
                        None));
                }
            },
                WindowEvent::KeyboardInput {
                input,
                ..
            } => {
                let scancode: input::keyboard::KeyScancode =
                    num::FromPrimitive::from_u8(input.scancode as u8)
                    .unwrap();
                let alt = modifiers.alt();
                #[allow(clippy::collapsible_if)]
                if !alt {
                    sys::enqueue_event(
                        sys::Event::new(Some(platform::get_msg_time()),
                        sys::EventType::Key(scancode, false),
                        None));
                        // toggle fullscreen on Alt+Enter
                } else if scancode == input::keyboard::KeyScancode::Enter {
                    if // (_DAT_02910164 != 8) &&
                    dvar::exists("r_fullscreen") &&
                        dvar::get_int("developer").unwrap() != 0 
                    {
                        // FUN_005a5360()
                        dvar::set_bool_internal(
                            "r_fullscreen", 
                            !dvar::get_bool("r_fullscreen")
                                .unwrap()).unwrap();
                            cbuf::add_textln(0, "vid_restart");
                    }
                        // FUN_0053f880()
                }
            },
            WindowEvent::Resized(size) => {
                dvar::make_latched_value_current("r_aspectRatio").unwrap();
                dvar::make_latched_value_current("r_aaSamples").unwrap();
                dvar::make_latched_value_current("r_vsync").unwrap();
                dvar::make_latched_value_current("r_fullscreen").unwrap();
                dvar::make_latched_value_current("r_displayRefresh").unwrap();
                let mut wnd_parms = gfx::WindowParms::new();
                let width = size.width;
                let height = size.height;
                let old_mode = dvar::get_enumeration("r_mode").unwrap();
                let new_mode = format!("{}x{}", width, height);
                dvar::add_to_enumeration_domain("r_mode", &new_mode).unwrap();
                dvar::set_enumeration_internal("r_mode", &new_mode).unwrap();
                dvar::remove_from_enumeration_domain("r_mode", &old_mode).unwrap();
                set_wnd_parms(&mut wnd_parms);
                store_window_settings(&mut wnd_parms).unwrap();
                set_wnd_parms(&mut wnd_parms);
                let lock = RENDER_GLOBALS.clone();
                let mut render_globals = lock.write().unwrap();
                render_globals.window.width = wnd_parms.display_width;
                render_globals.window.height = wnd_parms.display_height;
                if !wnd_parms.fullscreen {
                    com::println(8.into(), &format!("Resizing {} x {} window at ({}, {})", wnd_parms.display_width, wnd_parms.display_height, wnd_parms.x, wnd_parms.y))
                } else {
                    com::println(8.into(), &format!("Resizing {} x {} fullscreen at ({}, {})", wnd_parms.display_width, wnd_parms.display_height, wnd_parms.x, wnd_parms.y))
                }
            },
            _ => {}
        },
        _ => {}
    });
}

fn init_hardware(wnd_parms: &mut gfx::WindowParms) -> Result<(), ()> {
    store_window_settings(wnd_parms).unwrap();
    com::println(8.into(), "TODO: render::init_hardware");
    Ok(())
}

pub fn create_window(wnd_parms: &mut gfx::WindowParms) -> Result<(), ()> {
    com::println(8.into(), &format!(
        "{}: render::create_window()...",
        std::thread::current().name().unwrap_or("main")
    ));

    init_hardware(wnd_parms).unwrap();

    {
        let lock = WND_PARMS.clone();
        let mut writer = lock.write().expect("");
        *writer = *wnd_parms;
    }
    com::println(8.into(), &format!(
        "{}: written WND_PARMS.",
        std::thread::current().name().unwrap_or("main")
    ));

    com::println(8.into(), &format!(
        "{}: waiting for init...",
        std::thread::current().name().unwrap_or("main")
    ));

    {
        let lock = WINDOW_AWAITING_INIT.clone();
        let mut writer = lock.write().expect("");
        writer.send(()).unwrap();
    }

    let res = loop {
        {
            let lock = WINDOW_INITIALIZED.clone();
            let mut window_initialized = lock.write().unwrap();
            if window_initialized.try_acknowledge().is_some() {
                break window_initialized.get_state();
            }
        }
    };
    com::println(8.into(), &format!(
        "{}: init complete, res={:?}...",
        std::thread::current().name().unwrap_or("main"),
        res
    ));

    match res {
        Ok(_) => Ok(()),
        Err(_) => Err(()),
    }
}

fn init_systems() -> Result<(), ()> {
    Ok(())
}

lazy_static! {
    pub static ref WND_PARMS: Arc<RwLock<gfx::WindowParms>> =
        Arc::new(RwLock::new(Default::default()));
}

fn init_graphics_api() -> Result<(), ()> {
    com::println(8.into(), &format!(
        "{}: render::init_graphics_api()...",
        std::thread::current().name().unwrap_or("main")
    ));
    if RENDER_GLOBALS.clone().read().expect("").device.is_none() {
        if pre_create_window().is_err() {
            return Err(());
        }

        loop {
            let mut wnd_parms: gfx::WindowParms = gfx::WindowParms::new();
            set_wnd_parms(&mut wnd_parms);
            if create_window(&mut wnd_parms).is_err() {
                break;
            }
            if reduce_window_settings().is_err() {
                fatal_init_error("Couldn't initialize renderer")
            }
        }

        Ok(())
    } else {
        init_systems()
    }
}
