use core::{cell::Cell, cmp::Ordering};

use xash3d_client::{
    consts::{PITCH, ROLL, YAW},
    cvar::{Cvar, NO_FLAGS},
    ffi::common::vec3_t,
    input::{KeyButton, KeyButtonData},
    macros::{hook_command, hook_command_key},
    math::fabsf,
    prelude::*,
};

use crate::{
    export::{camera, input},
    helpers,
};

// const CAM_COMMAND_NONE: c_int = 0;
const CAM_COMMAND_TOTHIRDPERSON: i32 = 1;
const CAM_COMMAND_TOFIRSTPERSON: i32 = 2;

const CAM_DIST_DELTA: f32 = 1.0;
const CAM_ANGLE_DELTA: f32 = 2.5;
const CAM_ANGLE_SPEED: f32 = 2.5;
const CAM_MIN_DIST: f32 = 30.0;
const CAM_ANGLE_MOVE: f32 = 0.5;
// const MAX_ANGLE_DIFF: f32 = 10.0;
// const PITCH_MAX: f32 = 90.0;
// const PITCH_MIN: f32 = 0.0;
// const YAW_MAX: f32 = 135.0;
// const YAW_MIN: f32 = -135.0;

static CAM_PITCHUP: KeyButtonData = KeyButtonData::new();
static CAM_PITCHDOWN: KeyButtonData = KeyButtonData::new();
static CAM_YAWLEFT: KeyButtonData = KeyButtonData::new();
static CAM_YAWRIGHT: KeyButtonData = KeyButtonData::new();
static CAM_IN: KeyButtonData = KeyButtonData::new();
static CAM_OUT: KeyButtonData = KeyButtonData::new();

pub struct Camera {
    engine: ClientEngineRef,

    cam_thirdperson: Cell<bool>,
    cam_mousemove: Cell<bool>,
    cam_distancemove: Cell<bool>,
    cam_ofs: Cell<vec3_t>,

    cam_pitchup: KeyButton,
    cam_pitchdown: KeyButton,
    cam_yawleft: KeyButton,
    cam_yawright: KeyButton,
    cam_in: KeyButton,
    cam_out: KeyButton,

    cam_command: Cvar<i32>,
    cam_snapto: Cvar<bool>,
    cam_idealyaw: Cvar,
    cam_idealpitch: Cvar,
    cam_idealdist: Cvar,

    c_maxpitch: Cvar,
    c_minpitch: Cvar,
    c_maxyaw: Cvar,
    c_minyaw: Cvar,
    c_maxdistance: Cvar,
    c_mindistance: Cvar,
}

impl Camera {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_command_key!(engine, "campitchup", camera().cam_pitchup);
        hook_command_key!(engine, "campitchdown", camera().cam_pitchdown);
        hook_command_key!(engine, "camyawleft", camera().cam_yawleft);
        hook_command_key!(engine, "camyawright", camera().cam_yawright);
        hook_command_key!(engine, "camin", camera().cam_in);
        hook_command_key!(engine, "camout", camera().cam_out);

        hook_command!(engine, c"snapto", |_| camera().cam_snapto.toggle());
        hook_command!(engine, c"thirdperson", |_| camera().set_third_person());
        hook_command!(engine, c"firstperson", |_| camera().set_first_person());
        hook_command!(engine, c"+cammousemove", |_| camera().start_mouse_move());
        hook_command!(engine, c"-cammousemove", |_| camera().end_mouse_move());
        hook_command!(engine, c"+camdistance", |_| camera().start_distance());
        hook_command!(engine, c"-camdistance", |_| camera().end_distance());

        Self {
            engine,

            cam_thirdperson: Cell::default(),
            cam_mousemove: Cell::default(),
            cam_distancemove: Cell::default(),
            cam_ofs: Cell::default(),

            cam_pitchup: KeyButton::new(engine, &CAM_PITCHUP),
            cam_pitchdown: KeyButton::new(engine, &CAM_PITCHDOWN),
            cam_yawleft: KeyButton::new(engine, &CAM_YAWLEFT),
            cam_yawright: KeyButton::new(engine, &CAM_YAWRIGHT),
            cam_in: KeyButton::new(engine, &CAM_IN),
            cam_out: KeyButton::new(engine, &CAM_OUT),

            cam_command: engine.create_cvar(c"cam_command", c"0", NO_FLAGS).unwrap(),
            cam_snapto: engine.create_cvar(c"cam_snapto", c"0", NO_FLAGS).unwrap(),
            cam_idealyaw: engine.create_cvar(c"cam_idealyaw", c"0", NO_FLAGS).unwrap(),
            cam_idealpitch: engine
                .create_cvar(c"cam_idealpitch", c"0", NO_FLAGS)
                .unwrap(),
            cam_idealdist: engine
                .create_cvar(c"cam_idealdist", c"64", NO_FLAGS)
                .unwrap(),

            c_maxpitch: engine
                .create_cvar(c"c_maxpitch", c"90.0", NO_FLAGS)
                .unwrap(),
            c_minpitch: engine.create_cvar(c"c_minpitch", c"0.0", NO_FLAGS).unwrap(),
            c_maxyaw: engine.create_cvar(c"c_maxyaw", c"135.0", NO_FLAGS).unwrap(),
            c_minyaw: engine
                .create_cvar(c"c_minyaw", c"-135.0", NO_FLAGS)
                .unwrap(),
            c_maxdistance: engine
                .create_cvar(c"c_maxdistance", c"200.0", NO_FLAGS)
                .unwrap(),
            c_mindistance: engine
                .create_cvar(c"c_mindistance", c"30.0", NO_FLAGS)
                .unwrap(),
        }
    }

    pub fn is_third_person(&self) -> bool {
        if self.cam_thirdperson.get() || unsafe { helpers::g_iUser1 } != 0 {
            return true;
        }
        let player = self.engine.get_local_player();
        unsafe { helpers::g_iUser2 == (*player).index }
    }

    // pub fn is_first_person(&self) -> bool {
    //     !self.is_third_person()
    // }

    pub fn offset(&self) -> vec3_t {
        self.cam_ofs.get()
    }

    fn set_offset(&self, offset: vec3_t) {
        self.cam_ofs.set(offset);
    }

    pub fn set_third_person(&self) {
        let engine = self.engine;
        if engine.is_multiplayer() {
            return;
        }

        let viewangles = engine.get_view_angles();

        if !self.cam_thirdperson.get() {
            self.cam_thirdperson.set(true);

            let mut cam_ofs = vec3_t::ZERO;
            cam_ofs[YAW] = viewangles[YAW];
            cam_ofs[PITCH] = viewangles[PITCH];
            cam_ofs[ROLL] = CAM_MIN_DIST;
            self.set_offset(cam_ofs);
        }

        self.cam_command.set(0);
    }

    pub fn set_first_person(&self) {
        self.cam_thirdperson.set(false);
        self.cam_command.set(0);
    }

    fn start_mouse_move(&self) {
        if !self.is_third_person() {
            self.end_mouse_move();
            return;
        }

        if !self.cam_mousemove.get() {
            self.cam_mousemove.set(false);
            input().mouse_in_use(true);
        }
    }

    fn end_mouse_move(&self) {
        self.cam_mousemove.set(false);
        input().mouse_in_use(false);
    }

    fn start_distance(&self) {
        if !self.is_third_person() {
            self.end_distance();
            return;
        }

        if !self.cam_distancemove.get() {
            self.cam_distancemove.set(true);
            self.cam_mousemove.set(true);
            input().mouse_in_use(true);
        }
    }

    fn end_distance(&self) {
        self.cam_distancemove.set(false);
        self.cam_mousemove.set(false);
        input().mouse_in_use(false);
    }

    pub fn think(&mut self) {
        match self.cam_command.get() {
            CAM_COMMAND_TOTHIRDPERSON => self.set_third_person(),
            CAM_COMMAND_TOFIRSTPERSON => self.set_first_person(),
            _ => {}
        }

        if !self.cam_thirdperson.get() {
            return;
        }

        let engine = self.engine;
        let (mouse_x, mouse_y) = engine.get_mouse_position();
        let (center_x, center_y) = engine.get_window_center();

        let mut cam_angles = vec3_t::ZERO;
        cam_angles[PITCH] = self.cam_idealpitch.get();
        cam_angles[YAW] = self.cam_idealyaw.get();
        let mut dist = self.cam_idealdist.get();

        if self.cam_mousemove.get() && !self.cam_distancemove.get() {
            match mouse_x.cmp(&center_x) {
                Ordering::Greater => {
                    let c_maxyaw = self.c_maxyaw.get();
                    if cam_angles[YAW] < c_maxyaw {
                        let diff = mouse_x - center_x;
                        cam_angles[YAW] += CAM_ANGLE_MOVE * (diff / 2) as f32;
                    }
                    if cam_angles[YAW] > c_maxyaw {
                        cam_angles[YAW] = c_maxyaw;
                    }
                }
                Ordering::Less => {
                    let c_minyaw = self.c_minyaw.get();
                    if cam_angles[YAW] > c_minyaw {
                        let diff = center_x - mouse_x;
                        cam_angles[YAW] -= CAM_ANGLE_MOVE * (diff / 2) as f32;
                    }
                    if cam_angles[YAW] < c_minyaw {
                        cam_angles[YAW] = c_minyaw;
                    }
                }
                Ordering::Equal => {}
            }

            match mouse_y.cmp(&center_y) {
                Ordering::Greater => {
                    let c_maxpitch = self.c_maxpitch.get();
                    if cam_angles[PITCH] < c_maxpitch {
                        let diff = mouse_y - center_y;
                        cam_angles[PITCH] += CAM_ANGLE_MOVE * (diff / 2) as f32;
                    }
                    if cam_angles[PITCH] > c_maxpitch {
                        cam_angles[PITCH] = c_maxpitch;
                    }
                }
                Ordering::Less => {
                    let c_minpitch = self.c_minpitch.get();
                    if cam_angles[PITCH] > c_minpitch {
                        let diff = center_y - mouse_y;
                        cam_angles[PITCH] -= CAM_ANGLE_MOVE * (diff / 2) as f32;
                    }
                    if cam_angles[PITCH] < c_minpitch {
                        cam_angles[PITCH] = c_minpitch;
                    }
                }
                Ordering::Equal => {}
            }
        }

        if self.cam_pitchup.key_state() != 0.0 {
            cam_angles[PITCH] += CAM_ANGLE_DELTA;
        } else if self.cam_pitchdown.key_state() != 0.0 {
            cam_angles[PITCH] -= CAM_ANGLE_DELTA;
        }

        if self.cam_yawleft.key_state() != 0.0 {
            cam_angles[YAW] -= CAM_ANGLE_DELTA;
        } else if self.cam_yawright.key_state() != 0.0 {
            cam_angles[YAW] += CAM_ANGLE_DELTA;
        }

        if self.cam_in.key_state() != 0.0 {
            dist -= CAM_DIST_DELTA;
            if dist < CAM_MIN_DIST {
                cam_angles[PITCH] = 0.0;
                cam_angles[YAW] = 0.0;
                dist = CAM_MIN_DIST;
            }
        } else if self.cam_out.key_state() != 0.0 {
            dist += CAM_DIST_DELTA;
        }

        if self.cam_distancemove.get() {
            match mouse_y.cmp(&center_y) {
                Ordering::Greater => {
                    let c_maxdistance = self.c_maxdistance.get();
                    if dist < c_maxdistance {
                        let diff = mouse_y - center_y;
                        dist += CAM_DIST_DELTA * (diff / 2) as f32;
                    }
                    if dist > c_maxdistance {
                        dist = c_maxdistance;
                    }
                }
                Ordering::Less => {
                    let c_mindistance = self.c_mindistance.get();
                    if dist > c_mindistance {
                        let diff = center_y - mouse_y;
                        dist -= CAM_DIST_DELTA * (diff / 2) as f32;
                    }
                    if dist < c_mindistance {
                        dist = c_mindistance;
                    }
                }
                Ordering::Equal => {}
            }
        }

        self.cam_idealpitch.set(cam_angles[PITCH]);
        self.cam_idealyaw.set(cam_angles[YAW]);
        self.cam_idealdist.set(dist);

        let viewangles = engine.get_view_angles();
        cam_angles = self.offset();
        if self.cam_snapto.get() {
            cam_angles[YAW] = self.cam_idealyaw.get() + viewangles[YAW];
            cam_angles[PITCH] = self.cam_idealpitch.get() + viewangles[PITCH];
            cam_angles[2] = self.cam_idealdist.get();
        } else {
            if cam_angles[YAW] - viewangles[YAW] != self.cam_idealyaw.get() {
                let yaw = self.cam_idealyaw.get() + viewangles[YAW];
                cam_angles[YAW] = move_toward(cam_angles[YAW], yaw, CAM_ANGLE_SPEED);
            }

            if cam_angles[PITCH] - viewangles[PITCH] != self.cam_idealpitch.get() {
                let pitch = self.cam_idealpitch.get() + viewangles[PITCH];
                cam_angles[PITCH] = move_toward(cam_angles[PITCH], pitch, CAM_ANGLE_SPEED);
            }

            if fabsf(cam_angles[2] - self.cam_idealdist.get()) < 2.0 {
                cam_angles[2] = self.cam_idealdist.get();
            } else {
                cam_angles[2] += (self.cam_idealdist.get() - cam_angles[2]) / 4.0;
            }
        }

        self.set_offset(vec3_t::new(cam_angles[0], cam_angles[1], dist));
    }
}

fn move_toward(mut cur: f32, goal: f32, _maxspeed: f32) -> f32 {
    if cur != goal {
        if fabsf(cur - goal) > 180.0 {
            if cur < goal {
                cur += 360.0;
            } else {
                cur -= 360.0;
            }
        }

        if cur < goal {
            if cur < goal - 1.0 {
                cur += (goal - cur) / 4.0;
            } else {
                cur = goal;
            }
        } else if cur > goal + 1.0 {
            cur -= (cur - goal) / 4.0;
        } else {
            cur = goal;
        }
    }

    if cur < 0.0 {
        cur += 360.0;
    } else if cur >= 360.0 {
        cur -= 360.0;
    }

    cur
}
