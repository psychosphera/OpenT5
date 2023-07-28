use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use lazy_static::lazy_static;

use arrayvec::{ArrayString, ArrayVec};
use libc::c_void;
use windows::Win32::{Foundation::HMODULE, Graphics::Direct3D9::{IDirect3D9, IDirect3DDevice9, D3DFORMAT, D3DDISPLAYMODE, IDirect3DQuery9, D3DMULTISAMPLE_TYPE, IDirect3DSurface9, D3DTEXTUREFILTERTYPE}};

use crate::gfx;

pub const D3D_VENDOR_ID_NVIDIA: u32 = 0x10DE;

#[derive(Clone, Default, Debug)]
pub struct DxGlobals {
    hinst: Option<HMODULE>,
    pub d3d9: Option<IDirect3D9>,
    device: Option<Box<IDirect3DDevice9>>,
    pub adapter_index: u32,
    pub vendor_id: u32,
    adapter_native_is_valid: bool,
    adapter_native_width: i32,
    adapter_native_height: i32,
    adapter_fullscreen_width: i32,
    adapter_fullscreen_height: i32,
    supports_scene_null_render_target: bool,
    supports_int_z: bool,
    pub nv_initialized: bool,
    nv_stereo_activated: bool,
    nv_stereo_handle: Option<*mut c_void>,
    nv_depth_buffer_handle: Option<*mut c_void>,
    nv_float_z_buffer_handle: Option<*mut c_void>,
    resize_window: bool,
    depth_stencil_format: D3DFORMAT,
    display_mode_count: u32,
    display_modes: ArrayVec<D3DDISPLAYMODE, 256>,
    resolution_name_table: ArrayVec<String, 257>,
    refresh_rate_name_table: ArrayVec<String, 257>,
    mode_text: ArrayString<5120>,
    fence_pool: [Option<Box<IDirect3DQuery9>>; 8],
    next_fence: u32,
    gpu_sync: i32,
    gpu_count: i32,
    multi_sample_type: D3DMULTISAMPLE_TYPE,
    multi_sample_quality: u32,
    sun_sprite_sample: i32,
    sun_shadow_partition_res: i32,
    single_sample_depth_stencil_surface: Option<Box<IDirect3DSurface9>>,
    in_scene: bool,
    target_window_index: i32,
    window_count: i32,
    window: gfx::WindowTarget,
    flush_gpu_query: Option<Box<IDirect3DQuery9>>,
    gpu_sync_delay: u64,
    gpu_sync_start: u64,
    gpu_sync_end: u64,
    linear_non_mipped_min_filter: D3DTEXTUREFILTERTYPE,
    linear_non_mipped_mag_filter: D3DTEXTUREFILTERTYPE,
    linear_mipped_min_filter: D3DTEXTUREFILTERTYPE,
    linear_mipped_mag_filter: D3DTEXTUREFILTERTYPE,
    anisotropic_min_filter: D3DTEXTUREFILTERTYPE,
    anisotropic_mag_filter: D3DTEXTUREFILTERTYPE,
    linear_mipped_anisotropy: i32,
    anisotropy_for_2x: i32,
    anisotropy_for_4x: i32,
    mip_filter_mode: i32,
    mip_bias: u32,
    swap_fence: [Option<Box<IDirect3DQuery9>>; 4],
}

unsafe impl Send for DxGlobals {}
unsafe impl Sync for DxGlobals {}

lazy_static! {
    static ref DX: RwLock<DxGlobals> = RwLock::new(DxGlobals::default());
}

pub fn dx() -> RwLockReadGuard<'static, DxGlobals> {
    DX.read().unwrap()
}

pub fn dx_mut() -> RwLockWriteGuard<'static, DxGlobals> {
    DX.write().unwrap()
}

pub fn nv_use_shadow_null_color_render_target() -> bool {
    dx().nv_initialized
}

lazy_static! {
    static ref GFX_METRICS: RwLock<GfxMetrics> = RwLock::new(GfxMetrics::default());
}

pub fn gfx_metrics() -> RwLockReadGuard<'static, GfxMetrics> {
    GFX_METRICS.read().unwrap()
}

pub fn gfx_metrics_mut() -> RwLockWriteGuard<'static, GfxMetrics> {
    GFX_METRICS.write().unwrap()
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DxCapsResponse {
    Quit,
    Warn,
    Info,
    ForbidSm3,
}

#[derive(Copy, Clone, Debug)]
pub struct DxCapsCheckBits {
    pub offset: isize,
    pub set_bits: u32,
    pub clear_bits: u32,
    pub response: DxCapsResponse,
    pub message: &'static str,
}

#[derive(Copy, Clone, Debug)]
pub struct DxCapsCheckInteger {
    pub offset: isize,
    pub min: u32,
    pub max: u32,
    pub response: DxCapsResponse,
    pub message: &'static str,
}

#[derive(Copy, Clone, Debug)]
pub enum ShadowmapSamplerState {
    A,
    B,
}

#[derive(Copy, Clone, Debug)]
pub enum ShadowmapBuildTechType {
    Depth,
    Color,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct GfxMetrics {
    cubemap_shot_res: u16,
    cubemap_shot_pixel_border: u16,
    feedback_width: u16,
    feedback_height: u16,
    pub has_anisotropic_min_filter: bool,
    pub has_anisotropic_mag_filter: bool,
    pub max_anisotropy: i32,
    pub max_clip_planes: i32,
    pub has_hardware_shadowmap: bool,
    pub shadowmap_format_primary: D3DFORMAT,
    pub shadowmap_format_secondary: D3DFORMAT,
    pub shadowmap_build_tech_type: Option<ShadowmapBuildTechType>,
    pub shadowmap_sampler_state: Option<ShadowmapSamplerState>,
    pub slope_scale_depth_bias: bool,
    pub can_mip_cubemaps: bool,
    pub has_transparency_msaa: bool,
}