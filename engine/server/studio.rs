use core::{ffi::c_void, fmt, marker::PhantomData, ptr::NonNull, slice};

use bitflags::bitflags;
use xash3d_shared::{
    csz::CStrThin,
    ffi::{api::studio, common::vec3_t},
};

use crate::{entity::EntityVars, prelude::*};

#[cfg(feature = "save")]
use crate::save;

macro_rules! define_consts {
    (
        $(
            $( #[$attr:meta] )*
            const $name:ident = $index:expr;
        )*
    ) => {
        $(
            $( #[$attr] )*
            pub const $name: Self = Self($index);
        )*

        fn const_as_str(&self) -> Option<&str> {
            match *self {
                $(
                    Self::$name => Some(stringify!($name)),
                )*
                _ => None,
            }
        }
    };
}

bitflags! {
    #[derive(Copy, Clone, Default, Debug)]
    pub struct Motion: u32 {
        const X         = 1 << 0;
        const Y         = 1 << 1;
        const Z         = 1 << 2;
        const XR        = 1 << 3;
        const YR        = 1 << 4;
        const ZR        = 1 << 5;
        const LX        = 1 << 6;
        const LY        = 1 << 7;
        const LZ        = 1 << 8;
        const AX        = 1 << 9;
        const AY        = 1 << 10;
        const AZ        = 1 << 11;
        const AXR       = 1 << 12;
        const AYR       = 1 << 13;
        const AZR       = 1 << 14;
        const TYPES     = 0x7fff;

        const RLOOP     = 1 << 15;
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Activity(i32);

impl Activity {
    define_consts! {
        const RESET             = 0; // Set m_Activity to this invalid value to force a reset to m_IdealActivity
        const IDLE              = 1;
        const GUARD             = 2;
        const WALK              = 3;
        const RUN               = 4;
        const FLY               = 5; // Fly (and flap if appropriate)
        const SWIM              = 6;
        const HOP               = 7; // vertical jump
        const LEAP              = 8; // long forward jump
        const FALL              = 9;
        const LAND              = 10;
        const STRAFE_LEFT       = 11;
        const STRAFE_RIGHT      = 12;
        const ROLL_LEFT         = 13; // tuck and roll, left
        const ROLL_RIGHT        = 14; // tuck and roll, right
        const TURN_LEFT         = 15; // turn quickly left (stationary)
        const TURN_RIGHT        = 16; // turn quickly right (stationary)
        const CROUCH            = 17; // the act of crouching down from a standing position
        const CROUCHIDLE        = 18; // holding body in crouched position (loops)
        const STAND             = 19; // the act of standing from a crouched position
        const USE               = 20;
        const SIGNAL1           = 21;
        const SIGNAL2           = 22;
        const SIGNAL3           = 23;
        const TWITCH            = 24;
        const COWER             = 25;
        const SMALL_FLINCH      = 26;
        const BIG_FLINCH        = 27;
        const RANGE_ATTACK1     = 28;
        const RANGE_ATTACK2     = 29;
        const MELEE_ATTACK1     = 30;
        const MELEE_ATTACK2     = 31;
        const RELOAD            = 32;
        const ARM               = 33; // pull out gun, for instance
        const DISARM            = 34; // reholster gun
        const EAT               = 35; // monster chowing on a large food item (loop)
        const DIESIMPLE         = 36;
        const DIEBACKWARD       = 37;
        const DIEFORWARD        = 38;
        const DIEVIOLENT        = 39;
        const BARNACLE_HIT      = 40; // barnacle tongue hits a monster
        const BARNACLE_PULL     = 41; // barnacle is lifting the monster ( loop )
        const BARNACLE_CHOMP    = 42; // barnacle latches on to the monster
        const BARNACLE_CHEW     = 43; // barnacle is holding the monster in its mouth ( loop )
        const SLEEP             = 44;
        const INSPECT_FLOOR     = 45; // for active idles, look at something on or near the floor
        const INSPECT_WALL      = 46; // for active idles, look at something directly ahead of you ( doesn't HAVE to be a wall or on a wall )
        const IDLE_ANGRY        = 47; // alternate idle animation in which the monster is clearly agitated. (loop)
        const WALK_HURT         = 48; // limp  (loop)
        const RUN_HURT          = 49; // limp  (loop)
        const HOVER             = 50; // Idle while in flight
        const GLIDE             = 51; // Fly (don't flap)
        const FLY_LEFT          = 52; // Turn left in flight
        const FLY_RIGHT         = 53; // Turn right in flight
        const DETECT_SCENT      = 54; // this means the monster smells a scent carried by the air
        const SNIFF             = 55; // this is the act of actually sniffing an item in front of the monster
        const BITE              = 56; // some large monsters can eat small things in one bite. This plays one time, EAT loops.
        const THREAT_DISPLAY    = 57; // without attacking, monster demonstrates that it is angry. (Yell, stick out chest, etc )
        const FEAR_DISPLAY      = 58; // monster just saw something that it is afraid of
        const EXCITED           = 59; // for some reason, monster is excited. Sees something he really likes to eat, or whatever.
        const SPECIAL_ATTACK1   = 60; // very monster specific special attacks.
        const SPECIAL_ATTACK2   = 61;
        const COMBAT_IDLE       = 62; // agitated idle.
        const WALK_SCARED       = 63;
        const RUN_SCARED        = 64;
        const VICTORY_DANCE     = 65; // killed a player, do a victory dance.
        const DIE_HEADSHOT      = 66; // die, hit in head.
        const DIE_CHESTSHOT     = 67; // die, hit in chest
        const DIE_GUTSHOT       = 68; // die, hit in gut
        const DIE_BACKSHOT      = 69; // die, hit in back
        const FLINCH_HEAD       = 70;
        const FLINCH_CHEST      = 71;
        const FLINCH_STOMACH    = 72;
        const FLINCH_LEFTARM    = 73;
        const FLINCH_RIGHTARM   = 74;
        const FLINCH_LEFTLEG    = 75;
        const FLINCH_RIGHTLEG   = 76;
    }

    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> i32 {
        self.0
    }

    pub fn as_str(&self) -> Option<&str> {
        self.const_as_str()
    }
}

impl fmt::Display for Activity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.as_str() {
            Some(name) => write!(f, "Activity({}, {name})", self.into_raw()),
            None => write!(f, "Activity({})", self.into_raw()),
        }
    }
}

#[cfg(feature = "save")]
impl Save for Activity {
    fn save(&self, _: &mut save::SaveState, cur: &mut save::CursorMut) -> save::SaveResult<()> {
        cur.write_leb_i32(self.into_raw())
    }
}

#[cfg(feature = "save")]
impl Restore for Activity {
    fn restore(&mut self, _: &save::RestoreState, cur: &mut save::Cursor) -> save::SaveResult<()> {
        self.0 = cur.read_leb_i32()?;
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventKind(i32);

impl EventKind {
    pub const SPECIFIC: Self = Self(0);
    pub const SCRIPTED: Self = Self(1000);
    pub const SHARED: Self = Self(2000);
    pub const CLIENT: Self = Self(5000);

    define_consts! {
        /// Character is now dead.
        const SCRIPT_DEAD               = 1000;
        /// Does not allow interrupt.
        const SCRIPT_NOINTERRUPT        = 1001;
        /// Will allow interrupt.
        const SCRIPT_CANINTERRUPT       = 1002;
        /// Event now fires.
        const SCRIPT_FIREEVENT          = 1003;
        /// Play named wave file (body channel).
        const SCRIPT_SOUND              = 1004;
        /// Play named sentence.
        const SCRIPT_SENTENCE           = 1005;
        /// Leave the character in air at the end of the sequence (do not find the floor).
        const SCRIPT_INAIR              = 1006;
        /// Set the animation by name after the sequence completes.
        const SCRIPT_ENDANIMATION       = 1007;
        /// Play named wave file (voice channel).
        const SCRIPT_SOUND_VOICE        = 1008;
        /// Play sentence group 25% of the time.
        const SCRIPT_SENTENCE_RND1      = 1009;
        /// Bring back to life (for life/death sequences).
        const SCRIPT_NOT_DEAD           = 1010;

        const MONSTER_BODYDROP_LIGHT    = 2001;
        const MONSTER_BODYDROP_HEAVY    = 2002;
        const MONSTER_SWISHSOUND        = 2010;
    }

    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    pub const fn into_raw(self) -> i32 {
        self.0
    }

    pub const fn is_specific(&self) -> bool {
        let raw = self.into_raw();
        raw >= Self::SPECIFIC.into_raw() && raw < Self::SCRIPTED.into_raw()
    }

    pub const fn is_scripted(&self) -> bool {
        let raw = self.into_raw();
        raw >= Self::SCRIPTED.into_raw() && raw < Self::SHARED.into_raw()
    }

    pub const fn is_shared(&self) -> bool {
        let raw = self.into_raw();
        raw >= Self::SHARED.into_raw() && raw < Self::CLIENT.into_raw()
    }

    pub const fn is_client(&self) -> bool {
        let raw = self.into_raw();
        raw >= EventKind::CLIENT.into_raw()
    }

    pub const fn is_script_sound(&self) -> bool {
        matches!(*self, Self::SCRIPT_SOUND | Self::SCRIPT_SOUND_VOICE)
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.const_as_str() {
            Some(name) => write!(f, "event({}, {name})", self.into_raw()),
            None => write!(f, "event({})", self.into_raw()),
        }
    }
}

#[repr(transparent)]
pub struct Event {
    raw: studio::mstudioevent_s,
}

impl Event {
    pub fn raw(&self) -> &studio::mstudioevent_s {
        &self.raw
    }

    pub fn frame(&self) -> i32 {
        self.raw.frame
    }

    pub fn kind(&self) -> EventKind {
        EventKind::from_raw(self.raw.event)
    }

    pub fn options(&self) -> &CStrThin {
        unsafe { CStrThin::from_ptr(self.raw.options.as_ptr()) }
    }
}

#[derive(Copy, Clone)]
pub struct Sequence<'a> {
    model: Model<'a>,
    index: u32,
}

impl<'a> Sequence<'a> {
    const FLAG_LOOPING: i32 = 1 << 0;
    const FLAG_STATIC: i32 = 1 << 15;

    unsafe fn new(model: Model<'a>, index: usize) -> Self {
        Self {
            model,
            index: index as u32,
        }
    }

    fn engine(&self) -> ServerEngineRef {
        self.model.engine
    }

    pub fn raw(&self) -> &'a studio::mstudioseqdesc_s {
        unsafe {
            self.model
                .sequences_raw()
                .get_unchecked(self.index as usize)
        }
    }

    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub fn name(&self) -> &'a CStrThin {
        unsafe { CStrThin::from_ptr(self.raw().label.as_ptr()) }
    }

    pub fn fps(&self) -> f32 {
        self.raw().fps
    }

    pub fn flags(&self) -> i32 {
        self.raw().flags
    }

    pub fn is_looping(&self) -> bool {
        self.raw().flags & Self::FLAG_LOOPING != 0
    }

    pub fn is_static(&self) -> bool {
        self.raw().flags & Self::FLAG_STATIC != 0
    }

    pub fn activity(&self) -> Activity {
        Activity::from_raw(self.raw().activity)
    }

    pub fn activity_weight(&self) -> i32 {
        self.raw().actweight
    }

    pub fn events(&self) -> &'a [Event] {
        unsafe {
            let base = self.model.as_ptr();
            let data = base.byte_offset(self.raw().eventindex as isize);
            let len = self.raw().numevents as usize;
            slice::from_raw_parts(data.cast(), len)
        }
    }

    pub fn frames(&self) -> i32 {
        self.raw().numframes
    }

    // pub weightlistindex: i32,
    // pub iklockindex: i32,
    // pub motiontype: i32,
    // pub motionbone: i32,

    pub fn linear_movement(&self) -> vec3_t {
        self.raw().linearmovement
    }

    // pub autolayerindex: i32,
    // pub keyvalueindex: i32,

    pub fn bbmin(&self) -> vec3_t {
        self.raw().bbmin
    }

    pub fn bbmax(&self) -> vec3_t {
        self.raw().bbmax
    }

    // pub numblends: i32,
    // pub animindex: i32,
    // pub blendtype: [i32; 2usize],
    // pub blendstart: [vec_t; 2usize],
    // pub blendend: [vec_t; 2usize],
    // pub groupsize: [u8; 2usize],
    // pub numautolayers: u8,
    // pub numiklocks: u8,
    // pub seqgroup: i32,

    pub fn entry_node(&self) -> i32 {
        self.raw().entrynode
    }

    pub fn exit_node(&self) -> i32 {
        self.raw().exitnode
    }

    // pub nodeflags: u8,
    // pub cycleposeindex: u8,
    // pub fadeintime: u8,
    // pub fadeouttime: u8,
    // pub animdescindex: i32,

    pub fn framerate(&self) -> f32 {
        if self.frames() > 1 {
            256.0 * self.fps() / (self.frames() - 1) as f32
        } else {
            256.0
        }
    }

    pub fn ground_speed(&self) -> f32 {
        if self.frames() > 1 {
            self.linear_movement().length()
        } else {
            0.0
        }
    }

    pub fn precache(&self) {
        for event in self.events() {
            if event.kind() >= EventKind::CLIENT {
                // do not send client-side events to the server
                continue;
            }
            if !event.kind().is_script_sound() {
                continue;
            }
            let sound_name = event.options();
            if !sound_name.is_empty() {
                self.engine().precache_sound(sound_name);
            } else {
                error!(
                    "invalid sound event {} in sequence {}::{} (sound is {sound_name:?})",
                    event.kind(),
                    self.model.name(),
                    self.name(),
                );
            }
        }
    }

    pub fn animation_events(&self, mut start: f32, mut end: f32) -> impl Iterator<Item = &Event> {
        if self.frames() > 1 {
            let last_frame = (self.frames() - 1) as f32;
            start *= last_frame / 256.0;
            end *= last_frame / 256.0;
        } else {
            start = 0.0;
            end = 1.0;
        }

        self.events().iter().filter(move |event| {
            if !event.kind().is_client() {
                let frame = event.frame() as f32;
                if frame >= start && frame < end {
                    return true;
                }
                if self.is_looping()
                    && end >= (self.frames() - 1) as f32
                    && frame < end - (self.frames() + 1) as f32
                {
                    return true;
                }
            }
            false
        })
    }
}

#[repr(transparent)]
pub struct BoneController {
    raw: studio::mstudiobonecontroller_t,
}

impl BoneController {
    pub fn raw(&self) -> &studio::mstudiobonecontroller_t {
        &self.raw
    }

    // pub fn bone(&self) -> i32 {
    //     self.raw.bone
    // }

    pub fn ty(&self) -> Motion {
        Motion::from_bits_retain(self.raw.type_ as u32)
    }

    pub fn start(&self) -> f32 {
        self.raw.start
    }

    pub fn end(&self) -> f32 {
        self.raw.end
    }

    pub fn index(&self) -> i32 {
        self.raw.index
    }
}

#[derive(Copy, Clone)]
pub struct Model<'a> {
    engine: ServerEngineRef,
    base: NonNull<c_void>,
    phantom: PhantomData<&'a studio::studiohdr_s>,
}

impl<'a> Model<'a> {
    pub fn new(v: &'a EntityVars) -> Option<Self> {
        NonNull::new(v.engine().get_model_ptr(v)).map(|header| Self {
            engine: v.engine().engine_ref(),
            base: header,
            phantom: PhantomData,
        })
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.base.as_ptr()
    }

    pub fn header(&self) -> &'a studio::studiohdr_s {
        unsafe { &*self.as_ptr().cast::<studio::studiohdr_s>() }
    }

    // pub ident: i32,

    pub fn version(&self) -> i32 {
        self.header().version
    }

    pub fn name(&self) -> &CStrThin {
        unsafe { CStrThin::from_ptr(self.header().name.as_ptr()) }
    }

    // pub length: i32,

    pub fn eye_position(&self) -> vec3_t {
        self.header().eyeposition
    }

    pub fn min(&self) -> vec3_t {
        self.header().min
    }

    pub fn max(&self) -> vec3_t {
        self.header().max
    }

    pub fn bbmin(&self) -> vec3_t {
        self.header().bbmin
    }

    pub fn bbmax(&self) -> vec3_t {
        self.header().bbmax
    }

    pub fn flags(&self) -> i32 {
        self.header().flags
    }

    // pub numbones: i32,
    // pub boneindex: i32,

    pub fn bone_controllers(&self) -> &'a [BoneController] {
        let header = self.header();
        unsafe {
            let data = self.base.byte_offset(header.bonecontrollerindex as isize);
            let len = header.numbonecontrollers as usize;
            slice::from_raw_parts(data.as_ptr().cast(), len)
        }
    }

    pub fn set_bone_controller(
        &self,
        v: &EntityVars,
        controller: i32,
        mut value: f32,
    ) -> Option<f32> {
        let ctrl = self
            .bone_controllers()
            .iter()
            .find(|i| i.index() == controller)?;

        // wrap rotation controllers around 0 and 360 degrees
        if ctrl.ty().intersects(Motion::XR | Motion::YR | Motion::ZR) {
            // numas13: "ugly hack" from original code, is it buggy models or what?
            if ctrl.end() < ctrl.start() {
                value = -value;
            }

            if ctrl.start() + 359.0 >= ctrl.end() {
                if value > (ctrl.start() + ctrl.end()) * 0.5 + 180.0 {
                    value -= 360.0;
                }
                if value < (ctrl.start() + ctrl.end()) * 0.5 - 180.0 {
                    value += 360.0;
                }
            } else if value > 360.0 {
                value -= (value / 360.0) as i32 as f32 * 360.0;
            } else if value < 0.0 {
                value += (value / -360.0 + 1.0) as i32 as f32 * 360.0;
            }
        }

        let diff = ctrl.end() - ctrl.start();
        let setting = (255.0 * (value - ctrl.start()) / diff) as u8;
        v.with_controller(|mut s| {
            s[controller as usize] = setting;
            s
        });
        Some(setting as f32 * (1.0 / 255.0) * diff + ctrl.start())
    }

    // pub numhitboxes: i32,
    // pub hitboxindex: i32,

    pub fn sequences_raw(&self) -> &'a [studio::mstudioseqdesc_s] {
        let header = self.header();
        unsafe {
            let data = self.base.byte_offset(header.seqindex as isize);
            let len = header.numseq as usize;
            slice::from_raw_parts(data.as_ptr().cast(), len)
        }
    }

    pub fn sequences(&self) -> impl Iterator<Item = Sequence<'a>> {
        self.sequences_raw()
            .iter()
            .enumerate()
            .map(|(index, _)| unsafe { Sequence::new(*self, index) })
    }

    pub fn sequences_by_activity(&self, activity: Activity) -> impl Iterator<Item = Sequence<'a>> {
        self.sequences()
            .filter(move |seq| seq.activity() == activity)
    }

    pub fn sequence(&self, index: usize) -> Option<Sequence<'a>> {
        self.sequences_raw()
            .get(index)
            .map(|_| unsafe { Sequence::new(*self, index) })
    }

    /// Searches for a sequence with the given name.
    pub fn find_sequence(&self, name: impl AsRef<CStrThin>) -> Option<Sequence<'a>> {
        let name = name.as_ref();
        self.sequences().find(|seq| seq.name().eq_ignore_case(name))
    }

    /// Searches for a sequence with the given name.
    ///
    /// Returns `0` if not found.
    pub fn find_sequence_index(&self, name: impl AsRef<CStrThin>) -> usize {
        self.find_sequence(name).map_or(0, |seq| seq.index())
    }

    /// Searches for a sequence with the given activity.
    pub fn find_sequence_by_activity(&self, activity: Activity) -> Option<Sequence<'a>> {
        let engine = self.engine;
        let mut ret = None;
        let mut weight_total = 0;
        for seq in self.sequences_by_activity(activity) {
            weight_total += seq.activity_weight();
            if weight_total == 0 || engine.random_int(0, weight_total - 1) < seq.activity_weight() {
                ret = Some(seq);
            }
        }
        ret
    }

    /// Searches for a sequence with the given activity.
    ///
    /// Returns `0` if not found.
    pub fn find_sequence_index_by_activity(&self, activity: Activity) -> usize {
        self.find_sequence_by_activity(activity)
            .map_or(0, |seq| seq.index())
    }

    /// Searches for a heaviest sequence with the given activity.
    pub fn find_sequence_by_activity_heaviest(&self, activity: Activity) -> Option<Sequence<'a>> {
        self.sequences_by_activity(activity)
            .max_by_key(|seq| seq.activity_weight())
    }

    /// Searches for a heaviest sequence with the given activity.
    ///
    /// Returns `0` if not found.
    pub fn find_sequence_index_by_activity_heaviest(&self, activity: Activity) -> usize {
        self.find_sequence_by_activity_heaviest(activity)
            .map_or(0, |seq| seq.index())
    }

    pub fn precache_sequence_by_name(&self, name: &CStrThin) {
        if let Some(seq) = self.find_sequence(name) {
            seq.precache();
        }
    }

    // pub numseqgroups: i32,
    // pub seqgroupindex: i32,
    // pub numtextures: i32,
    // pub textureindex: i32,
    // pub texturedataindex: i32,
    // pub numskinref: i32,
    // pub numskinfamilies: i32,
    // pub skinindex: i32,
    // pub numbodyparts: i32,
    // pub bodypartindex: i32,
    // pub numattachments: i32,
    // pub attachmentindex: i32,
    // pub studiohdr2index: i32,

    pub fn transitions_raw(&self) -> &'a [u8] {
        let header = self.header();
        unsafe {
            let data = self.base.byte_offset(header.transitionindex as isize);
            let len = header.numtransitions as usize;
            slice::from_raw_parts(data.as_ptr().cast::<u8>(), len)
        }
    }
}
