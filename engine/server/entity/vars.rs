use core::{
    ffi::{CStr, c_int},
    fmt,
    num::NonZeroI32,
    ptr::{self, NonNull},
};

use bitflags::bitflags;
use xash3d_shared::{
    color::{RGB, RGBA},
    csz::CStrThin,
    entity::{Buttons, EdictFlags, Effects, EntityIndex, MoveType},
    ffi::{
        self,
        common::vec3_t,
        server::{edict_s, entvars_s},
    },
    macros::define_enum_for_primitive,
    math::ToAngleVectors,
    render::{RenderFx, RenderMode},
    str::ToEngineStr,
};

use crate::{
    entity::{EntityHandle, EntityOffset, KeyValue},
    prelude::*,
    save::{FieldType, SaveFields},
    str::MapString,
    studio::Model,
    time::MapTime,
};

trait RawBitflags<T> {
    fn from_bits_retain(bits: T) -> Self;

    fn bits(self) -> T;
}

impl RawBitflags<i32> for u32 {
    fn from_bits_retain(bits: i32) -> Self {
        bits as u32
    }

    fn bits(self) -> i32 {
        self as i32
    }
}

define_enum_for_primitive! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub enum Solid: i32 {
        #[default]
        Not(ffi::common::SOLID_NOT),
        Trigger(ffi::common::SOLID_TRIGGER),
        BBox(ffi::common::SOLID_BBOX),
        SlideBox(ffi::common::SOLID_SLIDEBOX),
        Bsp(ffi::common::SOLID_BSP),
        Custom(ffi::common::SOLID_CUSTOM),
        Portal(ffi::common::SOLID_PORTAL),
    }
}

define_enum_for_primitive! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub enum TakeDamage: f32 {
        #[default]
        No(Self::DAMAGE_NO),
        Yes(Self::DAMAGE_YES),
        Aim(Self::DAMAGE_AIM),
    }
}

impl TakeDamage {
    const DAMAGE_NO: f32 = ffi::common::DAMAGE_NO as f32;
    const DAMAGE_YES: f32 = ffi::common::DAMAGE_YES as f32;
    const DAMAGE_AIM: f32 = ffi::common::DAMAGE_AIM as f32;
}

define_enum_for_primitive! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub enum Dead: i32 {
        /// Alive.
        #[default]
        No(ffi::common::DEAD_NO),
        /// Playing death animation or still falling off of a ledge waiting to hit ground.
        Dying(ffi::common::DEAD_DYING),
        /// Dead. Lying still.
        Yes(ffi::common::DEAD_DEAD),
        Respawnable(ffi::common::DEAD_RESPAWNABLE),
        DiscardBody(ffi::common::DEAD_DISCARDBODY),
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct Weapons: u32 {
        const SUIT          = 1 << 31;
    }
}

define_enum_for_primitive! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub enum WaterLevel: i32 {
        // Not in water.
        #[default]
        Dry(0),
        // Feet underwater.
        Feet(1),
        // Waist underwater.
        Waist(2),
        // Head underwater (completely submerged).
        Head(3),
    }
}

macro_rules! field {
    // get enum
    (get enum $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> $ty:ty) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> $ty {
            let value = unsafe { (*self.as_ptr()).$field };
            <$ty>::from_raw(value).unwrap()
        }
    };
    // get bitflags with cast
    (get bitflags $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> $ty:ty, $from:ty as $to:ty) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> $ty {
            let value: $from = unsafe { (*self.as_ptr()).$field };
            <$ty>::from_bits_retain(value as $to)
        }
    };
    // get bitflags
    (get bitflags $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> $ty:ty) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> $ty {
            let value = unsafe { (*self.as_ptr()).$field };
            <$ty>::from_bits_retain(value)
        }
    };
    // get optional entity handle
    (get $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> Option<EntityHandle>) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> Option<EntityHandle> {
            unsafe {
                let value = (*self.as_ptr()).$field;
                EntityHandle::new(self.engine, value)
            }
        }
    };
    // get optional map string
    (get $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> Option<MapString>) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> Option<MapString> {
            let value = unsafe { (*self.as_ptr()).$field };
            MapString::from_index(self.engine, value)
        }
    };
    // get map time
    (get $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> MapTime) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> MapTime {
            let value = unsafe { (*self.as_ptr()).$field };
            MapTime::from_secs_f32(value)
        }
    };
    // get optional non-null pointer
    (get $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> Option<NonNull<$ty:ty>>) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> Option<NonNull<$ty>> {
            let value = unsafe { (*self.as_ptr()).$field };
            NonNull::new(value)
        }
    };
    // get type with cast to other type
    (get $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> $ty:ty as $to:ty) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> $to {
            let value: $ty = unsafe { (*self.as_ptr()).$field };
            value as $to
        }
    };
    // get field as is
    (get $field:ident, $( #[$attr:meta] )* fn $meth:ident() -> $ty:ty) => {
        $( #[$attr] )*
        pub fn $meth(&self) -> $ty {
            unsafe { (*self.as_ptr()).$field }
        }
    };

    // set entity
    (set entity $field:ident, $( #[$attr:meta] )* fn $meth:ident($arg:ident)) => {
        $( #[$attr] )*
        pub fn $meth<'a, T, U>(&self, $arg: T)
        where T: Into<Option<&'a U>>,
              U: 'a + ?Sized + AsEntityHandle,
        {
            let value = $arg.into().map_or(ptr::null_mut(), |v| v.as_entity_handle());
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };
    // set enum
    (set enum $field:ident, $( #[$attr:meta] )* fn $meth:ident($arg:ident: $ty:ty)) => {
        $( #[$attr] )*
        pub fn $meth(&self, $arg: $ty) {
            let value = $arg.into_raw();
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };
    // set bitflags
    (set bitflags $field:ident, $( #[$attr:meta] )* fn $meth:ident($arg:ident: $ty:ty $( as $to:ty)?)) => {
        $( #[$attr] )*
        pub fn $meth(&self, $arg: $ty) {
            let value = $arg.bits() $( as $to)?;
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };
    // set optional map string
    (set $field:ident, $( #[$attr:meta] )* fn $meth:ident($arg:ident: Option<MapString>)) => {
        $( #[$attr] )*
        pub fn $meth(&self, $arg: impl Into<Option<MapString>>) {
            let value = $arg.into().map_or(0, |s| s.index());
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };
    // set map time
    (set $field:ident, $( #[$attr:meta] )* fn $meth:ident($arg:ident: MapTime)) => {
        $( #[$attr] )*
        pub fn $meth(&self, $arg: MapTime) {
            let value = $arg.as_secs_f32();
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };
    // set vector
    (set $field:ident, $( #[$attr:meta] )* fn $meth:ident($arg:ident: vec3_t)) => {
        $( #[$attr] )*
        pub fn $meth(&self, $arg: impl Into<vec3_t>) {
            let value = $arg.into();
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };
    // set field as is
    (set $field:ident, $( #[$attr:meta] )* fn $meth:ident($arg:ident: $ty:ty)) => {
        $( #[$attr] )*
        pub fn $meth(&self, $arg: $ty) {
            let value = $arg;
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };

    // unsafe set field as is
    (set $field:ident, $( #[$attr:meta] )* unsafe fn $meth:ident($arg:ident: $ty:ty)) => {
        $( #[$attr] )*
        pub unsafe fn $meth(&self, $arg: $ty) {
            let value = $arg;
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };

    // modify bitflags
    (mut bitflags $field:ident, $( #[$attr:meta] )* fn $meth:ident($ty:ty $(, $from:ty as $to:ty)?)) => {
        $( #[$attr] )*
        pub fn $meth(&self, f: impl FnOnce($ty) -> $ty) {
            let bits = unsafe { (*self.as_ptr()).$field };
            $( let bits: $from = bits; )?
            $( let bits: $to = bits as $to; )?
            let value = f(<$ty>::from_bits_retain(bits)).bits();
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };
    // modify field
    (mut $field:ident, $( #[$attr:meta] )* fn $meth:ident($ty:ty)) => {
        $( #[$attr] )*
        pub fn $meth(&self, f: impl FnOnce($ty) -> $ty) {
            let value = unsafe { (*self.as_ptr()).$field };
            let value = f(value);
            unsafe {
                (*self.as_mut_ptr()).$field = value;
            }
        }
    };
}

/// A safe wrapper for [entvars_s].
#[derive(PartialEq, Eq)]
pub struct EntityVars {
    engine: ServerEngineRef,
    global_state: GlobalStateRef,
    raw: NonNull<entvars_s>,
}

impl EntityVars {
    /// Creates a wrapper from a raw pointer.
    ///
    /// # Safety
    ///
    /// Behavior is undefined if any of the following conditions are violated:
    ///
    /// * The raw pointer must be non-null and received from the engine.
    pub unsafe fn from_raw(
        engine: ServerEngineRef,
        global_state: GlobalStateRef,
        raw: *mut entvars_s,
    ) -> Self {
        Self {
            engine,
            global_state,
            raw: unsafe { NonNull::new_unchecked(raw) },
        }
    }

    pub fn as_ptr(&self) -> *const entvars_s {
        self.raw.as_ptr()
    }

    pub fn as_mut_ptr(&self) -> *mut entvars_s {
        self.raw.as_ptr()
    }

    pub fn engine(&self) -> ServerEngineRef {
        self.engine
    }

    pub fn global_state(&self) -> GlobalStateRef {
        self.global_state
    }

    /// Returns an entity handle of this entity.
    pub fn entity_handle(&self) -> EntityHandle {
        unsafe { EntityHandle::new_unchecked(self.engine, self.containing_entity_raw()) }
    }

    /// Returns an index of this entity.
    pub fn entity_index(&self) -> EntityIndex {
        self.engine().get_entity_index(self)
    }

    /// Returns an offset of this entity.
    pub fn entity_offset(&self) -> EntityOffset {
        self.engine().get_entity_offset(self)
    }

    pub fn pretty_name(&self) -> super::PrettyName<'_> {
        super::PrettyName::new(self)
    }

    field!(get classname, fn classname() -> Option<MapString>);
    field!(set classname, fn set_classname(s: Option<MapString>));

    pub fn is_class_name(&self, name: impl AsRef<CStrThin>) -> bool {
        self.classname()
            .is_some_and(|s| s.as_thin() == name.as_ref())
    }

    field!(get globalname, fn globalname() -> Option<MapString>);
    field!(set globalname, fn set_globalname(s: Option<MapString>));

    pub fn is_global_name(&self, name: impl AsRef<CStrThin>) -> bool {
        self.globalname()
            .is_some_and(|s| s.as_thin() == name.as_ref())
    }

    field!(get origin, fn origin() -> vec3_t);
    field!(set origin,
        /// Sets a new world position of this entity without linking to the world.
        ///
        /// Call [EntityVars::link] to link the entity to the list.
        fn set_origin(v: vec3_t));
    field!(mut origin, fn with_origin(vec3_t));

    /// Links this entity into the list.
    pub fn link(&self) {
        let engine = self.engine();
        engine.set_origin_and_link(self.origin(), self);
    }

    /// Sets a new world position of this entity and links it into the list.
    pub fn set_origin_and_link(&self, origin: impl Into<vec3_t>) {
        self.set_origin(origin);
        self.link();
    }

    field!(get oldorigin, fn old_origin() -> vec3_t);
    field!(set oldorigin, fn set_old_origin(v: vec3_t));
    field!(mut oldorigin, fn with_old_origin(vec3_t));

    field!(get velocity, fn velocity() -> vec3_t);
    field!(set velocity, fn set_velocity(v: vec3_t));
    field!(mut velocity, fn with_velocity(vec3_t));

    field!(get basevelocity, fn base_velocity() -> vec3_t);
    field!(set basevelocity, fn set_base_velocity(v: vec3_t));
    field!(mut basevelocity, fn with_base_velocity(vec3_t));

    field!(get clbasevelocity, fn client_base_velocity() -> vec3_t);
    field!(set clbasevelocity, fn set_client_base_velocity(v: vec3_t));
    field!(mut clbasevelocity, fn with_client_base_velocity(vec3_t));

    field!(get movedir, fn move_dir() -> vec3_t);
    field!(set movedir, fn set_move_dir(v: vec3_t));
    field!(mut movedir, fn with_move_dir(vec3_t));

    pub fn set_move_dir_from_angles(&self) {
        let ev = unsafe { &mut (*self.as_mut_ptr()) };
        if ev.angles == vec3_t::new(0.0, -1.0, 0.0) {
            ev.movedir = vec3_t::new(0.0, 0.0, 1.0);
        } else if ev.angles == vec3_t::new(0.0, -2.0, 0.0) {
            ev.movedir = vec3_t::new(0.0, 0.0, -1.0);
        } else {
            ev.movedir = ev.angles.angle_vectors().forward();
        }
        ev.angles = vec3_t::ZERO;
    }

    pub fn set_move_dir_from_axis(&self, x: bool, z: bool) {
        if z {
            self.set_move_dir(vec3_t::Z);
        } else if x {
            self.set_move_dir(vec3_t::X);
        } else {
            self.set_move_dir(vec3_t::Y);
        }
    }

    pub fn set_move_dir_from_spawn_flags(&self, x_bit: u32, z_bit: u32) {
        let sf = self.spawn_flags();
        self.set_move_dir_from_axis(sf & x_bit != 0, sf & z_bit != 0);
    }

    // pub fn set_move_dir_from_spawn_flags_default(&self) {
    //     const SF_ROTATE_Z: u32 = 1 << 6;
    //     const SF_ROTATE_X: u32 = 1 << 7;
    //     self.set_move_dir_from_spawn_flags(SF_ROTATE_X, SF_ROTATE_Z);
    // }

    field!(get angles, fn angles() -> vec3_t);
    field!(set angles, fn set_angles(v: vec3_t));
    field!(mut angles, fn with_angles(vec3_t));

    field!(get avelocity, fn angular_velocity() -> vec3_t);
    field!(set avelocity, fn set_angular_velocity(v: vec3_t));
    field!(mut avelocity, fn with_angular_velocity(vec3_t));

    field!(get punchangle, fn punch_angle() -> vec3_t);
    field!(set punchangle, fn set_punch_angle(v: vec3_t));
    field!(mut punchangle, fn with_punch_angle(vec3_t));

    field!(get v_angle, fn view_angle() -> vec3_t);
    field!(set v_angle, fn set_view_angle(v: vec3_t));
    field!(mut v_angle, fn with_view_angle(vec3_t));

    field!(get endpos, fn end_pos() -> vec3_t);
    field!(set endpos, fn set_end_pos(v: vec3_t));
    field!(mut endpos, fn with_end_pos(vec3_t));

    field!(get startpos, fn start_pos() -> vec3_t);
    field!(set startpos, fn set_start_pos(v: vec3_t));
    field!(mut startpos, fn with_start_pos(vec3_t));

    field!(get impacttime, fn impact_time() -> f32);
    field!(set impacttime, fn set_impact_time(v: f32));

    field!(get starttime, fn start_time() -> f32);
    field!(set starttime, fn set_start_time(v: f32));

    // TODO: fixangle: define enum or wrapper???
    // 0: nothing
    // 1: force view angles
    // 2: add angular velocity
    field!(get fixangle, fn fix_angle() -> i32);
    field!(set fixangle, fn set_fix_angle(v: i32));

    field!(get idealpitch, fn ideal_pitch() -> f32);
    field!(set idealpitch, fn set_ideal_pitch(v: f32));

    field!(get pitch_speed, fn pitch_speed() -> f32);
    field!(set pitch_speed, fn set_pitch_speed(v: f32));

    field!(get ideal_yaw, fn ideal_yaw() -> f32);
    field!(set ideal_yaw, fn set_ideal_yaw(v: f32));

    field!(get yaw_speed, fn yaw_speed() -> f32);
    field!(set yaw_speed, fn set_yaw_speed(v: f32));

    field!(get modelindex, fn model_index_raw() -> i32);
    field!(set modelindex, fn set_model_index_raw(v: i32));

    pub fn model_index(&self) -> Option<NonZeroI32> {
        NonZeroI32::new(self.model_index_raw())
    }

    field!(get model, fn model_name_raw() -> i32);
    field!(set model, fn set_model_name_raw(v: i32));
    field!(get model, fn model_name() -> Option<MapString>);
    field!(set model, fn set_model_name(v: Option<MapString>));

    pub fn remove_model(&self) {
        self.set_model_name(None);
        self.set_model_index_raw(0);
    }

    pub fn model(&self) -> Option<Model<'_>> {
        Model::new(self)
    }

    pub fn set_model(&self, name: impl ToEngineStr) {
        let engine = self.engine;
        engine.set_model(self, name)
    }

    pub fn precache_model(&self) {
        if let Some(model_name) = self.model_name() {
            self.engine.precache_model(model_name);
        }
    }

    pub fn set_model_with_precache(&self, name: impl ToEngineStr) {
        let engine = self.engine;
        let name = name.to_engine_str();
        engine.precache_model(name.as_ref());
        engine.set_model(self, name.as_ref())
    }

    pub fn reload_model(&self) {
        if let Some(name) = self.model_name() {
            self.set_model(name);
        }
    }

    pub fn reload_model_with_precache(&self) {
        if let Some(name) = self.model_name() {
            self.set_model_with_precache(name);
        }
    }

    field!(get viewmodel, fn view_model_name_raw() -> i32);
    field!(set viewmodel, fn set_view_model_name_raw(v: i32));
    field!(get viewmodel, fn view_model_name() -> Option<MapString>);
    field!(set viewmodel, fn set_view_model_name(v: Option<MapString>));

    field!(get weaponmodel, fn weapon_model_name_raw() -> i32);
    field!(set weaponmodel, fn set_weapon_model_name_raw(v: i32));
    field!(get weaponmodel, fn weapon_model_name() -> Option<MapString>);
    field!(set weaponmodel, fn set_weapon_model_name(v: Option<MapString>));

    field!(get absmin, fn abs_min() -> vec3_t);
    field!(set absmin, fn set_abs_min(v: vec3_t));
    field!(mut absmin, fn with_abs_min(vec3_t));

    field!(get absmax, fn abs_max() -> vec3_t);
    field!(set absmax, fn set_abs_max(v: vec3_t));
    field!(mut absmax, fn with_abs_max(vec3_t));

    /// Returns an absolute center position in the world.
    pub fn abs_center(&self) -> vec3_t {
        (self.abs_min() + self.abs_max()) * 0.5
    }

    field!(get mins, fn min_size() -> vec3_t);
    field!(set mins, fn set_min_size(v: vec3_t));
    field!(mut mins, fn with_min_size(vec3_t));

    field!(get maxs, fn max_size() -> vec3_t);
    field!(set maxs, fn set_max_size(v: vec3_t));
    field!(mut maxs, fn with_max_size(vec3_t));

    /// Returns a center position in the entity coordinates.
    pub fn center(&self) -> vec3_t {
        (self.min_size() + self.max_size()) * 0.5
    }

    field!(get size, fn size() -> vec3_t);
    field!(set size, fn set_size(v: vec3_t));
    field!(mut size, fn with_size(vec3_t));

    pub fn set_size_and_link(&self, min: vec3_t, max: vec3_t) {
        self.engine.set_size(self, min, max);
    }

    pub fn bmodel_origin(&self) -> vec3_t {
        self.abs_min() + self.size() * 0.5
    }

    field!(get ltime, fn last_think_time() -> MapTime);
    field!(set ltime, fn set_last_think_time(time: MapTime));

    pub fn set_last_think_time_from_now(&self, relative: f32) {
        let now = self.engine.globals.map_time();
        self.set_last_think_time(now + relative);
    }

    field!(get nextthink, fn next_think_time() -> MapTime);
    field!(set nextthink, fn set_next_think_time(time: MapTime));

    /// Sets the next think time relative to the map time.
    pub fn set_next_think_time_from_now(&self, relative: f32) {
        self.set_next_think_time(self.engine.globals.map_time() + relative)
    }

    /// Sets the next think time relative to the last think time.
    pub fn set_next_think_time_from_last(&self, relative: f32) {
        self.set_next_think_time(self.last_think_time() + relative);
    }

    pub fn stop_thinking(&self) {
        self.set_next_think_time(MapTime::from_secs_f32(-1.0));
    }

    pub fn is_thinking(&self) -> bool {
        //  0.0 think was not called (the default value)
        // -1.0 called stop_thinking
        self.next_think_time() > MapTime::ZERO
    }

    field!(get movetype, fn move_type_raw() -> i32);
    field!(set movetype, fn set_move_type_raw(v: i32));
    field!(get enum movetype, fn move_type() -> MoveType);
    field!(set enum movetype, fn set_move_type(v: MoveType));

    field!(get solid, fn solid_raw() -> i32);
    field!(set solid, fn set_solid_raw(v: i32));
    field!(get enum solid, fn solid() -> Solid);
    field!(set enum solid, fn set_solid(v: Solid));

    field!(get skin, fn skin() -> i32);
    field!(set skin, fn set_skin(v: i32));

    field!(get body, fn body() -> i32);
    field!(set body, fn set_body(v: i32));

    field!(get effects, fn effects_raw() -> i32);
    field!(set effects, fn set_effects_raw(v: i32));
    field!(mut effects, fn with_effects_raw(i32));
    field!(get bitflags effects, fn effects() -> Effects);
    field!(set bitflags effects, fn set_effects(v: Effects));
    field!(mut bitflags effects, fn with_effects(Effects));

    pub fn remove_effects(&self) {
        self.set_effects(Effects::NONE);
    }

    field!(get gravity, fn gravity() -> f32);
    field!(set gravity, fn set_gravity(v: f32));

    field!(get friction, fn friction() -> f32);
    field!(set friction, fn set_friction(v: f32));

    // TODO: pfnGetEntityIllum???
    // field!(get light_level, fn light_level() -> i32);
    // field!(set light_level, fn set_light_level(v: i32));

    field!(get sequence, fn sequence() -> i32);
    field!(set sequence, fn set_sequence(v: i32));

    field!(get gaitsequence, fn gaitsequence() -> i32);
    field!(set gaitsequence, fn set_gaitsequence(v: i32));

    field!(get frame, fn frame() -> f32);
    field!(set frame, fn set_frame(v: f32));

    field!(get animtime, fn animation_time() -> f32);
    field!(set animtime, fn set_animation_time(v: f32));

    field!(get framerate, fn framerate() -> f32);
    field!(set framerate, fn set_framerate(v: f32));

    field!(get controller, fn controller() -> [u8; 4]);
    field!(set controller, fn set_controller(v: [u8; 4]));
    field!(mut controller, fn with_controller([u8; 4]));

    field!(get blending, fn blending() -> [u8; 2]);
    field!(set blending, fn set_blending(v: [u8; 2]));
    field!(mut blending, fn with_blending([u8; 2]));

    field!(get scale,
        /// Returns this entity rendering scale. Applies to studio and sprite models.
        fn scale() -> f32);
    field!(set scale, fn set_scale(v: f32));

    field!(get rendermode, fn render_mode_raw() -> i32);
    field!(set rendermode, fn set_render_mode_raw(v: i32));
    field!(get enum rendermode, fn render_mode() -> RenderMode);
    field!(set enum rendermode, fn set_render_mode(v: RenderMode));

    field!(get renderamt, fn render_amount() -> f32);
    field!(set renderamt, fn set_render_amount(v: f32));

    field!(get rendercolor, fn render_color() -> vec3_t);
    field!(set rendercolor, fn set_render_color(v: vec3_t));

    pub fn render_color_to_rgb(&self) -> RGB {
        let &[r, g, b] = self.render_color().as_ref();
        RGB::new(r as u8, g as u8, b as u8)
    }

    pub fn set_render_color_from_rgb(&self, color: RGB) {
        self.set_render_color(vec3_t::new(
            color.r() as f32,
            color.g() as f32,
            color.b() as f32,
        ))
    }

    pub fn render_color_to_rgba(&self) -> RGBA {
        self.render_color_to_rgb().rgba(self.render_amount() as u8)
    }

    pub fn set_render_color_from_rgba(&self, color: RGBA) {
        self.set_render_color_from_rgb(color.into());
        self.set_render_amount(color.a() as f32);
    }

    field!(get renderfx, fn render_fx_raw() -> i32);
    field!(set renderfx, fn set_render_fx_raw(v: i32));
    field!(get enum renderfx, fn render_fx() -> RenderFx);
    field!(set enum renderfx, fn set_render_fx(v: RenderFx));

    field!(get health, fn health() -> f32);
    field!(set health, fn set_health(v: f32));
    field!(mut health, fn with_health(f32));

    field!(get frags, fn frags() -> f32);
    field!(set frags, fn set_frags(v: f32));

    // TODO: use bitflags weapons?
    field!(get bitflags weapons, fn weapons() -> u32);
    field!(set bitflags weapons, fn set_weapons(v: u32));
    field!(mut bitflags weapons, fn with_weapons(u32));

    pub fn has_suit(&self) -> bool {
        Weapons::from_bits_retain(self.weapons()).intersects(Weapons::SUIT)
    }

    field!(get takedamage, fn take_damage_raw() -> f32);
    field!(set takedamage, fn set_take_damage_raw(v: f32));
    field!(get enum takedamage, fn take_damage() -> TakeDamage);
    field!(set enum takedamage, fn set_take_damage(v: TakeDamage));

    field!(get deadflag, fn dead_raw() -> i32);
    field!(set deadflag, fn set_dead_raw(v: i32));
    field!(get enum deadflag, fn dead() -> Dead);
    field!(set enum deadflag, fn set_dead(v: Dead));

    field!(get view_ofs, fn view_ofs() -> vec3_t);
    field!(set view_ofs, fn set_view_ofs(v: vec3_t));
    field!(mut view_ofs, fn with_view_ofs(vec3_t));

    field!(get button, fn buttons_raw() -> i32);
    field!(set button, fn set_buttons_raw(v: i32));
    field!(mut button, fn with_buttons_raw(i32));
    field!(get bitflags button, fn buttons() -> Buttons);
    field!(set bitflags button, fn set_buttons(v: Buttons));
    field!(mut bitflags button, fn with_buttons(Buttons));

    field!(get bitflags impulse, fn impulse() -> u32);
    field!(set bitflags impulse, fn set_impulse(v: u32));
    field!(mut bitflags impulse, fn with_impulse(u32));

    field!(get chain, fn chain() -> Option<EntityHandle>);
    field!(set entity chain, fn set_chain(chain));

    field!(get dmg_inflictor, fn damage_inflictor() -> Option<EntityHandle>);
    field!(set entity dmg_inflictor, fn set_damage_inflictor(entity));

    field!(get enemy, fn enemy() -> Option<EntityHandle>);
    field!(set entity enemy, fn set_enemy(enemy));

    field!(get aiment, fn aim_entity() -> Option<EntityHandle>);
    field!(set entity aiment, fn set_aim_entity(owner));

    field!(get owner, fn owner() -> Option<EntityHandle>);
    field!(set entity owner, fn set_owner(owner));

    field!(get groundentity, fn ground_entity() -> Option<EntityHandle>);
    field!(set entity groundentity, fn set_ground_entity(ground));

    field!(get bitflags spawnflags, fn spawn_flags() -> u32);
    field!(set bitflags spawnflags, fn set_spawn_flags(v: u32));
    field!(mut bitflags spawnflags, fn with_spawn_flags(u32));

    field!(get flags, fn flags_raw() -> i32);
    field!(set flags, fn set_flags_raw(v: i32));
    field!(mut flags, fn with_flags_raw(i32));
    field!(get bitflags flags, fn flags() -> EdictFlags);
    field!(set bitflags flags, fn set_flags(v: EdictFlags));
    field!(mut bitflags flags, fn with_flags(EdictFlags));

    field!(get colormap, fn color_map() -> i32);
    field!(set colormap, fn set_color_map(v: i32));

    field!(get team, fn team() -> i32);
    field!(set team, fn set_team(v: i32));

    field!(get max_health, fn max_health() -> f32);
    field!(set max_health, fn set_max_health(v: f32));

    field!(get teleport_time, fn teleport_time() -> MapTime);
    field!(set teleport_time, fn set_teleport_time(v: MapTime));

    field!(get armortype, fn armor_type() -> f32);
    field!(set armortype, fn set_armor_type(v: f32));

    field!(get armorvalue, fn armor_value() -> f32);
    field!(set armorvalue, fn set_armor_value(v: f32));

    field!(get waterlevel, fn water_level_raw() -> i32);
    field!(set waterlevel, fn set_water_level_raw(v: i32));
    field!(get enum waterlevel, fn water_level() -> WaterLevel);
    field!(set enum waterlevel, fn set_water_level(v: WaterLevel));

    // TODO: define WaterType(CONTENTS_*) enum for watertype???
    field!(get watertype, fn water_type() -> i32);
    field!(set watertype, fn set_water_type(v: i32));

    field!(get target, fn target_raw() -> i32);
    field!(set target, fn set_target_raw(s: i32));
    field!(get target, fn target() -> Option<MapString>);
    field!(set target, fn set_target(s: Option<MapString>));

    pub fn is_target(&self, name: impl AsRef<CStrThin>) -> bool {
        self.target().is_some_and(|s| s.as_thin() == name.as_ref())
    }

    field!(get targetname, fn target_name_raw() -> i32);
    field!(set targetname, fn set_target_name_raw(s: i32));
    field!(get targetname, fn target_name() -> Option<MapString>);
    field!(set targetname, fn set_target_name(s: Option<MapString>));

    pub fn is_target_name(&self, name: impl AsRef<CStrThin>) -> bool {
        self.target_name()
            .is_some_and(|s| s.as_thin() == name.as_ref())
    }

    field!(get netname, fn net_name_raw() -> i32);
    field!(set netname, fn set_net_name_raw(s: i32));
    field!(get netname, fn net_name() -> Option<MapString>);
    field!(set netname, fn set_net_name(s: Option<MapString>));

    field!(get message, fn message_raw() -> i32);
    field!(set message, fn set_message_raw(s: i32));
    field!(get message, fn message() -> Option<MapString>);
    field!(set message, fn set_message(s: Option<MapString>));

    field!(get dmg_take, fn damage_take() -> f32);
    field!(set dmg_take, fn set_damage_take(v: f32));

    field!(get dmg_save, fn damage_save() -> f32);
    field!(set dmg_save, fn set_damage_save(v: f32));

    field!(get dmg, fn damage() -> f32);
    field!(set dmg, fn set_damage(v: f32));

    field!(get dmgtime,
        /// Returns the map time at which this entity last took damage.
        fn damage_time() -> MapTime);
    field!(set dmgtime, fn set_damage_time(v: MapTime));

    field!(get noise, fn noise_raw() -> i32);
    field!(set noise, fn set_noise_raw(s: i32));
    field!(get noise, fn noise() -> Option<MapString>);
    field!(set noise, fn set_noise(s: Option<MapString>));

    field!(get noise1, fn noise1_raw() -> i32);
    field!(set noise1, fn set_noise1_raw(s: i32));
    field!(get noise1, fn noise1() -> Option<MapString>);
    field!(set noise1, fn set_noise1(s: Option<MapString>));

    field!(get noise2, fn noise2_raw() -> i32);
    field!(set noise2, fn set_noise2_raw(s: i32));
    field!(get noise2, fn noise2() -> Option<MapString>);
    field!(set noise2, fn set_noise2(s: Option<MapString>));

    field!(get noise3, fn noise3_raw() -> i32);
    field!(set noise3, fn set_noise3_raw(s: i32));
    field!(get noise3, fn noise3() -> Option<MapString>);
    field!(set noise3, fn set_noise3(s: Option<MapString>));

    field!(get speed, fn speed() -> f32);
    field!(set speed, fn set_speed(v: f32));

    field!(get air_finished, fn air_finished_time() -> MapTime);
    field!(set air_finished, fn set_air_finished_time(v: MapTime));

    field!(get pain_finished, fn pain_finished_time() -> MapTime);
    field!(set pain_finished, fn set_pain_finished_time(v: MapTime));

    field!(get radsuit_finished, fn radsuit_finished_time() -> MapTime);
    field!(set radsuit_finished, fn set_radsuit_finished_time(v: MapTime));

    field!(get pContainingEntity, fn containing_entity_raw() -> *mut edict_s);
    field!(set pContainingEntity,
        #[allow(clippy::missing_safety_doc)]
        unsafe fn set_containing_entity_raw(v: *mut edict_s));

    field!(get playerclass, fn player_class() -> i32);
    field!(set playerclass, fn set_player_class(v: i32));

    field!(get maxspeed, fn max_speed() -> f32);
    field!(set maxspeed, fn set_max_speed(v: f32));

    field!(get fov, fn fov() -> f32);
    field!(set fov, fn set_fov(v: f32));

    field!(get weaponanim, fn weapon_animation() -> i32);
    field!(set weaponanim, fn set_weapon_animation(v: i32));

    field!(get pushmsec, fn push_msec() -> i32);
    field!(set pushmsec, fn set_push_msec(v: i32));

    field!(get bInDuck, fn in_duck_raw() -> i32);
    field!(set bInDuck, fn set_in_duck_raw(v: i32));

    pub fn in_duck(&self) -> bool {
        self.in_duck_raw() != 0
    }

    pub fn set_in_duck(&self, v: bool) {
        self.set_in_duck_raw(v as i32)
    }

    field!(get flTimeStepSound, fn time_step_sound() -> i32);
    field!(set flTimeStepSound, fn set_time_step_sound(v: i32));

    field!(get flSwimTime, fn swim_time() -> i32);
    field!(set flSwimTime, fn set_swim_time(v: i32));

    field!(get flDuckTime, fn duck_time() -> i32);
    field!(set flDuckTime, fn set_duck_time(v: i32));

    field!(get iStepLeft, fn step_left() -> i32);
    field!(set iStepLeft, fn set_step_left(v: i32));

    field!(get flFallVelocity, fn fall_velocity() -> f32);
    field!(set flFallVelocity, fn set_fall_velocity(v: f32));

    field!(get gamestate, fn game_state() -> i32);
    field!(set gamestate, fn set_game_state(v: i32));

    field!(get oldbuttons, fn old_buttons_raw() -> i32);
    field!(set oldbuttons, fn set_old_buttons_raw(v: i32));
    field!(mut oldbuttons, fn with_old_buttons_raw(i32));
    field!(get bitflags oldbuttons, fn old_buttons() -> Buttons);
    field!(set bitflags oldbuttons, fn set_old_buttons(v: Buttons));
    field!(mut bitflags oldbuttons, fn with_old_buttons(Buttons));

    field!(get groupinfo, fn group_info() -> i32);
    field!(set groupinfo, fn set_group_info(v: i32));

    field!(get iuser1, fn iuser1() -> i32);
    field!(set iuser1, fn set_iuser1(v: i32));
    field!(mut iuser1, fn with_iuser1(i32));

    field!(get iuser2, fn iuser2() -> i32);
    field!(set iuser2, fn set_iuser2(v: i32));
    field!(mut iuser2, fn with_iuser2(i32));

    field!(get iuser3, fn iuser3() -> i32);
    field!(set iuser3, fn set_iuser3(v: i32));
    field!(mut iuser3, fn with_iuser3(i32));

    field!(get iuser4, fn iuser4() -> i32);
    field!(set iuser4, fn set_iuser4(v: i32));
    field!(mut iuser4, fn with_iuser4(i32));

    field!(get fuser1, fn fuser1() -> f32);
    field!(set fuser1, fn set_fuser1(v: f32));
    field!(mut fuser1, fn with_fuser1(f32));

    field!(get fuser2, fn fuser2() -> f32);
    field!(set fuser2, fn set_fuser2(v: f32));

    field!(get fuser3, fn fuser3() -> f32);
    field!(set fuser3, fn set_fuser3(v: f32));

    field!(get fuser4, fn fuser4() -> f32);
    field!(set fuser4, fn set_fuser4(v: f32));

    field!(get vuser1, fn vuser1() -> vec3_t);
    field!(set vuser1, fn set_vuser1(v: vec3_t));
    field!(mut vuser1, fn with_vuser1(vec3_t));

    field!(get vuser2, fn vuser2() -> vec3_t);
    field!(set vuser2, fn set_vuser2(v: vec3_t));
    field!(mut vuser2, fn with_vuser2(vec3_t));

    field!(get vuser3, fn vuser3() -> vec3_t);
    field!(set vuser3, fn set_vuser3(v: vec3_t));
    field!(mut vuser3, fn with_vuser3(vec3_t));

    field!(get vuser4, fn vuser4() -> vec3_t);
    field!(set vuser4, fn set_vuser4(v: vec3_t));
    field!(mut vuser4, fn with_vuser4(vec3_t));

    field!(get euser1, fn euser1_raw() -> *mut edict_s);
    field!(set euser1,
        #[allow(clippy::missing_safety_doc)]
        unsafe fn set_euser1_raw(ent: *mut edict_s));

    field!(get euser2, fn euser2_raw() -> *mut edict_s);
    field!(set euser2,
        #[allow(clippy::missing_safety_doc)]
        unsafe fn set_euser2_raw(ent: *mut edict_s));

    field!(get euser3, fn euser3_raw() -> *mut edict_s);
    field!(set euser3,
        #[allow(clippy::missing_safety_doc)]
        unsafe fn set_euser3_raw(ent: *mut edict_s));

    field!(get euser4, fn euser4_raw() -> *mut edict_s);
    field!(set euser4,
        #[allow(clippy::missing_safety_doc)]
        unsafe fn set_euser4_raw(ent: *mut edict_s));

    field!(get euser1, fn euser1() -> Option<EntityHandle>);
    field!(set entity euser1, fn set_euser1(ent));

    field!(get euser2, fn euser2() -> Option<EntityHandle>);
    field!(set entity euser2, fn set_euser2(ent));

    field!(get euser3, fn euser3() -> Option<EntityHandle>);
    field!(set entity euser3, fn set_euser3(ent));

    field!(get euser4, fn euser4() -> Option<EntityHandle>);
    field!(set entity euser4, fn set_euser4(ent));

    /// Ask the engine to remove this entity at the appropriate time.
    pub fn delayed_remove(&self) {
        self.with_flags(|f| f | EdictFlags::KILLME);
        self.set_target_name(None);
    }

    /// Call [Entity::remove_from_world](super::Entity::remove_from_world) if this entity vars have
    /// private data or call [EntityVars::delayed_remove] if not.
    pub fn remove_from_world(&self) {
        if let Some(entity) = self.get_entity() {
            entity.remove_from_world();
        } else {
            self.delayed_remove();
        }
    }

    pub(crate) unsafe fn key_value(&self, data: &mut KeyValue) {
        let key_name = data.key_name();

        if key_name == c"damage" {
            self.set_damage(data.value_str().parse().unwrap_or(0.0));
            data.set_handled(true);
            return;
        }

        let field = entvars_s::SAVE_FIELDS.iter().find(|i| {
            let name = unsafe { CStrThin::from_ptr(i.fieldName) };
            name.eq_ignore_case(key_name)
        });

        if let Some(field) = field {
            let field_type = FieldType::from_raw(field.fieldType).unwrap();
            let pev = self.as_ptr() as *mut u8;
            let p = unsafe { pev.offset(field.fieldOffset as isize) };

            match field_type {
                FieldType::MODELNAME | FieldType::SOUNDNAME | FieldType::STRING => {
                    let s = self.engine.new_map_string(data.value());
                    unsafe {
                        ptr::write(p.cast::<c_int>(), s.index());
                    }
                }
                FieldType::TIME | FieldType::FLOAT => {
                    let v = data.value_str().parse().unwrap_or(0.0);
                    unsafe {
                        ptr::write(p.cast::<f32>(), v);
                    }
                }
                FieldType::INTEGER => {
                    let v = data.value_str().parse().unwrap_or(0);
                    unsafe {
                        ptr::write(p.cast::<c_int>(), v);
                    }
                }
                FieldType::POSITION_VECTOR | FieldType::VECTOR => {
                    let mut v = vec3_t::ZERO;
                    for (i, s) in data.value_str().split(' ').enumerate() {
                        v[i] = s.parse().unwrap_or(0.0);
                    }
                    unsafe {
                        ptr::write(p.cast::<vec3_t>(), v);
                    }
                }
                _ => {
                    let name = unsafe { CStr::from_ptr(field.fieldName) };
                    error!(
                        "EntityVars::key_value: unimplemented field type {} for {name:?}",
                        field.fieldType
                    );
                }
            }
            data.set_handled(true);
        }
    }
}

impl fmt::Debug for EntityVars {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EntityVars").finish()
    }
}
