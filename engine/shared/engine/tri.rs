use core::{
    ffi::c_int,
    mem::{self, MaybeUninit},
};

use crate::{
    color::RGBA,
    ffi::{
        self,
        api::tri::triangleapi_s,
        common::{model_s, vec3_t},
    },
    macros::define_enum_for_primitive,
    render::RenderMode,
};

define_enum_for_primitive! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum Primitive: i32 {
        Points(ffi::api::tri::TRI_POINTS),
        Lines(ffi::api::tri::TRI_LINES),
        Triangles(ffi::api::tri::TRI_TRIANGLES),
        TriangleStrip(ffi::api::tri::TRI_TRIANGLE_STRIP),
        TriangleFan(ffi::api::tri::TRI_TRIANGLE_FAN),
        Quads(ffi::api::tri::TRI_QUADS),
        QuadStrip(ffi::api::tri::TRI_QUAD_STRIP),
        Polygon(ffi::api::tri::TRI_POLYGON),
    }
}

define_enum_for_primitive! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum CullFace: u32 {
        None(ffi::api::tri::TRICULLSTYLE_TRI_NONE),
        Front(ffi::api::tri::TRICULLSTYLE_TRI_FRONT),
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ScreenCoord {
    Front(vec3_t),
    Back(vec3_t),
}

macro_rules! unwrap {
    ($self:expr, $name:ident) => {
        match $self.raw.$name {
            Some(func) => func,
            None => panic!("triangleapi_s.{} is null", stringify!($name)),
        }
    };
}

static mut DRAW_LOCK: bool = false;

pub struct Draw<'a> {
    tri: &'a TriangleApi,
}

impl<'a> Draw<'a> {
    unsafe fn begin_unchecked(tri: &'a TriangleApi, primitive: Primitive) -> Self {
        unsafe {
            unwrap!(tri, Begin)(primitive.into_raw());
        }
        Self { tri }
    }

    pub fn begin(tri: &'a TriangleApi, primitive: Primitive) -> Self {
        unsafe {
            if DRAW_LOCK {
                panic!("multiple draw streams");
            }
            DRAW_LOCK = true;
            Self::begin_unchecked(tri, primitive)
        }
    }

    unsafe fn end_unchecked_impl(&self) {
        unsafe {
            unwrap!(self.tri, End)();
        }
    }

    // unsafe fn end_unchecked(self) {
    //     unsafe {
    //         self.end_unchecked_impl();
    //     }
    //     // do not call drop
    //     mem::forget(self);
    // }

    pub fn end(self) {
        // drop will do the work
    }

    pub fn color4f(self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.tri.set_color4f(r, g, b, a);
        self
    }

    pub fn color4ub(self, r: u8, g: u8, b: u8, a: u8) -> Self {
        self.tri.set_color4ub(r, g, b, a);
        self
    }

    pub fn color(self, color: impl Into<RGBA>) -> Self {
        self.tri.set_color(color.into());
        self
    }

    pub fn brightness(self, brightness: f32) -> Self {
        self.tri.set_brightness(brightness);
        self
    }

    pub fn tex_coord2f(self, u: f32, v: f32) -> Self {
        unsafe {
            unwrap!(self.tri, TexCoord2f)(u, v);
        }
        self
    }

    pub fn vertex3fv(self, pos: vec3_t) -> Self {
        unsafe {
            unwrap!(self.tri, Vertex3fv)(pos.as_ref().as_ptr());
        }
        self
    }

    pub fn vertex3f(self, x: f32, y: f32, z: f32) -> Self {
        unsafe {
            unwrap!(self.tri, Vertex3f)(x, y, z);
        }
        self
    }

    // TODO: do we need Color4fRendermode?
    // pub Color4fRendermode: Option<
    //     unsafe extern "C" fn(r: f32, g: f32, b: f32, a: f32, rendermode: ::core::ffi::c_int),
    // >,
}

impl Drop for Draw<'_> {
    fn drop(&mut self) {
        unsafe {
            self.end_unchecked_impl();
            DRAW_LOCK = false;
        }
    }
}

#[repr(transparent)]
pub struct TriangleApi {
    raw: triangleapi_s,
}

impl TriangleApi {
    #[doc(hidden)]
    pub fn new(raw: &triangleapi_s) -> &Self {
        // SAFETY: the struct has repr(transparent)
        unsafe { mem::transmute(raw) }
    }

    pub fn raw(&self) -> &triangleapi_s {
        &self.raw
    }

    pub fn version(&self) -> i32 {
        self.raw.version
    }

    pub fn set_render_mode(&self, mode: RenderMode) {
        unsafe { unwrap!(self, RenderMode)(mode.into_raw()) }
    }

    pub fn set_cull_face(&self, style: CullFace) {
        unsafe { unwrap!(self, CullFace)(style.into_raw()) }
    }

    pub fn set_color4f(&self, r: f32, g: f32, b: f32, a: f32) {
        unsafe { unwrap!(self, Color4f)(r, g, b, a) }
    }

    pub fn set_color4ub(&self, r: u8, g: u8, b: u8, a: u8) {
        unsafe { unwrap!(self, Color4ub)(r, g, b, a) }
    }

    pub fn set_color(&self, color: impl Into<RGBA>) {
        let color: RGBA = color.into();
        self.set_color4ub(color.r(), color.g(), color.b(), color.a());
    }

    pub fn set_brightness(&self, brightness: f32) {
        unsafe {
            unwrap!(self, Brightness)(brightness);
        }
    }

    pub fn set_sprite_texture(&self, model: &model_s, frame: i32) -> bool {
        let status = unsafe {
            unwrap!(self, SpriteTexture)(
                // FIXME: ffi: why model is mutable?
                (model as *const model_s).cast_mut(),
                frame,
            )
        };
        status != 0
    }

    pub fn begin(&self, primitive: Primitive) -> Draw<'_> {
        Draw::begin(self, primitive)
    }

    pub fn world_to_screen(&self, world: vec3_t) -> ScreenCoord {
        let mut ret = MaybeUninit::<[f32; 3]>::uninit();
        let behind = unsafe {
            unwrap!(self, WorldToScreen)(world.as_ref().as_ptr(), ret.as_mut_ptr().cast())
        };
        // SAFETY: the engine method initialize the value
        let ret = unsafe { ret.assume_init().into() };
        if behind == 0 {
            ScreenCoord::Front(ret)
        } else {
            ScreenCoord::Back(ret)
        }
    }

    pub fn screen_to_world(&self, screen: vec3_t) -> vec3_t {
        let mut ret = MaybeUninit::<[f32; 3]>::uninit();
        unsafe {
            unwrap!(self, ScreenToWorld)(screen.as_ref().as_ptr(), ret.as_mut_ptr().cast());
        }
        // SAFETY: the engine method initialize the value
        unsafe { ret.assume_init().into() }
    }

    pub fn get_matrix_raw(&self, param_name: i32) -> [f32; 16] {
        let mut ret = [0.0; 16];
        unsafe {
            unwrap!(self, GetMatrix)(param_name, ret.as_mut_ptr());
        }
        // numas13: the engine probably returns a matrix with the column-major ordering layout
        ret
    }

    pub fn is_box_in_pvs(&self, mins: vec3_t, maxs: vec3_t) -> bool {
        let mins = mins.as_ref().as_ptr();
        let maxs = maxs.as_ref().as_ptr();
        // FIXME: ffi: why mins and maxs are mutable?
        unsafe { unwrap!(self, BoxInPVS)(mins.cast_mut(), maxs.cast_mut()) != 0 }
    }

    pub fn light_at_point(&self, pos: vec3_t) -> [f32; 3] {
        let pos = pos.as_ref().as_ptr();
        let mut color = MaybeUninit::<[f32; 3]>::uninit();
        unsafe {
            // FIXME: ffi: why pos is mutable?
            unwrap!(self, LightAtPoint)(pos.cast_mut(), color.as_mut_ptr().cast());
        }
        // SAFETY: the engine method initialize the value
        unsafe { color.assume_init() }
    }

    pub fn fog_params(&self, density: f32, skybox: bool) {
        unsafe { unwrap!(self, FogParams)(density, skybox as c_int) }
    }

    pub fn fog(&self, color: &[f32; 3], start: f32, end: f32, on: bool) {
        unsafe {
            unwrap!(self, Fog)(
                // FIXME: ffi: why color is mutable?
                (color as *const [f32; 3]).cast_mut(),
                start,
                end,
                on as c_int,
            )
        }
    }
}
