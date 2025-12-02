use core::{
    cell::Cell,
    ffi::{CStr, c_int},
    mem,
};

use alloc::{boxed::Box, vec::Vec};
use xash3d_client::{
    consts::{self, PITCH, ROLL, YAW},
    csz::{CStrBox, CStrThin},
    cvar::{self, Cvar},
    ffi::{
        common::{kbutton_t, usercmd_s, vec3_t},
        keys,
    },
    input::{KeyButton, KeyState},
    macros::{hook_command, hook_command_key},
    math::{angle_mod, pow, sqrt, sqrtf},
    prelude::*,
};

use crate::{
    export::{hud, input, view},
    helpers,
    hud::Hud,
    view::View,
};

const MOUSE_BUTTON_COUNT: c_int = 5;

struct Mouse {}

impl Mouse {
    fn new() -> Self {
        Self {}
    }
}

struct Key {
    name: CStrBox,
    kb: *mut kbutton_t,
}

pub struct KeyList {
    list: Vec<Key>,
}

impl KeyList {
    fn new() -> Self {
        Self { list: Vec::new() }
    }

    pub fn find(&self, name: &CStrThin) -> Option<*mut kbutton_t> {
        self.list.iter().find(|i| i.name == *name).map(|i| i.kb)
    }

    fn add(&mut self, name: &CStr, kb: *mut kbutton_t) {
        if self.find(name.into()).is_some() {
            return;
        }

        self.list.push(Key {
            name: name.into(),
            kb,
        })
    }
}

pub struct Input {
    engine: ClientEngineRef,

    pub keys: KeyList,
    oldangles: vec3_t,
    #[allow(dead_code)]
    mouse: Mouse,

    in_impulse: Cell<u8>,
    in_cancel: Cell<bool>,

    mouse_initialized: bool,
    mouse_active: bool,
    mouse_visible: bool,
    mouse_in_use: Cell<bool>,
    mouse_raw_used: bool,

    mouse_buttons: c_int,
    mouse_oldbuttonstate: c_int,
    old_mouse_x: c_int,
    old_mouse_y: c_int,
    mx_accum: c_int,
    my_accum: c_int,
    mouse_x: f32,
    mouse_y: f32,

    in_graph: Box<KeyButton>,
    in_mlook: Box<KeyButton>,
    in_jlook: Box<KeyButton>,
    in_klook: KeyButton,
    in_left: KeyButton,
    in_right: KeyButton,
    in_forward: KeyButton,
    in_back: KeyButton,
    in_lookup: KeyButton,
    in_lookdown: KeyButton,
    in_moveleft: KeyButton,
    in_moveright: KeyButton,
    in_strafe: KeyButton,
    in_speed: KeyButton,
    in_use: KeyButton,
    in_jump: KeyButton,
    in_attack: KeyButton,
    in_attack2: KeyButton,
    in_up: KeyButton,
    in_down: KeyButton,
    in_duck: KeyButton,
    in_reload: KeyButton,
    in_alt1: KeyButton,
    in_score: KeyButton,
    in_break: KeyButton,

    lookstrafe: Cvar<bool>,
    lookspring: Cvar<bool>,
    cl_pitchup: Cvar,
    cl_pitchdown: Cvar,
    cl_pitchspeed: Cvar,
    cl_anglespeedkey: Cvar,
    cl_yawspeed: Cvar,
    cl_upspeed: Cvar,
    cl_forwardspeed: Cvar,
    cl_backspeed: Cvar,
    cl_sidespeed: Cvar,
    cl_movespeedkey: Cvar,

    m_pitch: Cvar,
    m_yaw: Cvar,
    m_forward: Cvar,
    m_side: Cvar,

    m_rawinput: Cvar<bool>,
    m_filter: Cvar<bool>,
    sensitivity: Cvar,

    m_customaccel: Cvar,
    m_customaccel_scale: Cvar<f64>,
    m_customaccel_max: Cvar,
    m_customaccel_exponent: Cvar<f64>,
}

impl Input {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_command_key!(engine, "moveup", input().in_up);
        hook_command_key!(engine, "movedown", input().in_down);
        hook_command_key!(engine, "left", input().in_left);
        hook_command_key!(engine, "right", input().in_right);
        hook_command_key!(engine, "forward", input().in_forward);
        hook_command_key!(engine, "back", input().in_back);
        hook_command_key!(engine, "lookup", input().in_lookup);
        hook_command_key!(engine, "lookdown", input().in_lookdown);
        hook_command_key!(engine, "strafe", input().in_strafe);
        hook_command_key!(engine, "moveleft", input().in_moveleft);
        hook_command_key!(engine, "moveright", input().in_moveright);
        hook_command_key!(engine, "speed", input().in_speed);
        hook_command_key!(engine, "attack", input().in_attack, up {
            input().in_cancel.set(false);
        });
        hook_command_key!(engine, "attack2", input().in_attack2);
        hook_command_key!(engine, "use", input().in_use);
        hook_command_key!(engine, "jump", input().in_jump);
        hook_command_key!(engine, "klook", input().in_klook);
        hook_command_key!(engine, "mlook", input().in_mlook, up {
            let input = input();
            let state = input.in_mlook.state();
            if !state.contains(KeyState::DOWN) && input.lookspring.get() {
                view().start_pitch_drift();
            }
        });
        hook_command_key!(engine, "jlook", input().in_jlook);
        hook_command_key!(engine, "duck", input().in_duck);
        hook_command_key!(engine, "reload", input().in_reload);
        hook_command_key!(engine, "alt1", input().in_alt1);
        hook_command_key!(engine, "score", input().in_score, down {
            hud().show_score_board(true);
        }, up {
            hud().show_score_board(false);
        });
        hook_command_key!(engine, "showscores", input().in_score, down {
            hud().show_score_board(true);
        }, up {
            hud().show_score_board(false);
        });
        hook_command_key!(engine, "graph", input().in_graph);
        hook_command_key!(engine, "break", input().in_break);

        // TODO: hook in_cancel???

        hook_command!(engine, c"impulse", |engine| {
            let s = engine.cmd_argv(1);
            let n = s.to_str().ok().and_then(|s| s.parse().ok()).unwrap_or(0);
            input().in_impulse.set(n);
        });

        hook_command!(engine, c"force_centerview", |engine| {
            if !input().mouse_in_use.get() {
                let mut viewangles = engine.get_view_angles();
                viewangles[PITCH] = 0.0;
                engine.set_view_angles(viewangles);
            }
        });

        hook_command!(engine, "joyadvancedupdate", |_| {
            // TODO: joystick
        });

        let in_graph = Box::new(KeyButton::new(engine));
        let in_mlook = Box::new(KeyButton::new(engine));
        let in_jlook = Box::new(KeyButton::new(engine));

        let mut keys = KeyList::new();
        keys.add(c"in_graph", (*in_graph).as_ptr());
        keys.add(c"in_mlook", (*in_mlook).as_ptr());
        keys.add(c"in_jlook", (*in_jlook).as_ptr());

        Self {
            engine,

            keys,
            oldangles: vec3_t::ZERO,
            mouse: Mouse::new(),

            in_impulse: Cell::new(0),
            in_cancel: Cell::new(false),

            mouse_initialized: !engine.check_parm(c"-nomouse"),
            mouse_active: false,
            mouse_visible: false,
            mouse_in_use: Cell::new(false),
            mouse_raw_used: true,

            mouse_buttons: MOUSE_BUTTON_COUNT,
            mouse_oldbuttonstate: 0,
            old_mouse_x: 0,
            old_mouse_y: 0,
            mx_accum: 0,
            my_accum: 0,
            mouse_x: 0.0,
            mouse_y: 0.0,

            in_graph,
            in_mlook,
            in_jlook,
            in_klook: KeyButton::new(engine),
            in_left: KeyButton::new(engine),
            in_right: KeyButton::new(engine),
            in_forward: KeyButton::new(engine),
            in_back: KeyButton::new(engine),
            in_lookup: KeyButton::new(engine),
            in_lookdown: KeyButton::new(engine),
            in_moveleft: KeyButton::new(engine),
            in_moveright: KeyButton::new(engine),
            in_strafe: KeyButton::new(engine),
            in_speed: KeyButton::new(engine),
            in_use: KeyButton::new(engine),
            in_jump: KeyButton::new(engine),
            in_attack: KeyButton::new(engine),
            in_attack2: KeyButton::new(engine),
            in_up: KeyButton::new(engine),
            in_down: KeyButton::new(engine),
            in_duck: KeyButton::new(engine),
            in_reload: KeyButton::new(engine),
            in_alt1: KeyButton::new(engine),
            in_score: KeyButton::new(engine),
            in_break: KeyButton::new(engine),

            lookstrafe: engine
                .create_cvar(c"lookstrafe", c"0", cvar::ARCHIVE)
                .unwrap(),
            lookspring: engine
                .create_cvar(c"lookspring", c"0", cvar::ARCHIVE)
                .unwrap(),
            cl_pitchup: engine
                .create_cvar(c"cl_pitchup", c"89", cvar::NO_FLAGS)
                .unwrap(),
            cl_pitchdown: engine
                .create_cvar(c"cl_pitchdown", c"89", cvar::NO_FLAGS)
                .unwrap(),
            cl_pitchspeed: engine
                .create_cvar(c"cl_pitchspeed", c"225", cvar::NO_FLAGS)
                .unwrap(),
            cl_anglespeedkey: engine
                .create_cvar(c"cl_anglespeedkey", c"0.67", cvar::NO_FLAGS)
                .unwrap(),
            cl_yawspeed: engine
                .create_cvar(c"cl_yawspeed", c"210", cvar::NO_FLAGS)
                .unwrap(),
            cl_upspeed: engine
                .create_cvar(c"cl_upspeed", c"320", cvar::NO_FLAGS)
                .unwrap(),
            cl_forwardspeed: engine
                .create_cvar(c"cl_forwardspeed", c"400", cvar::ARCHIVE)
                .unwrap(),
            cl_backspeed: engine
                .create_cvar(c"cl_backspeed", c"400", cvar::ARCHIVE)
                .unwrap(),
            cl_sidespeed: engine
                .create_cvar(c"cl_sidespeed", c"400", cvar::NO_FLAGS)
                .unwrap(),
            cl_movespeedkey: engine
                .create_cvar(c"cl_movespeedkey", c"0.3", cvar::NO_FLAGS)
                .unwrap(),

            m_pitch: engine
                .create_cvar(c"m_pitch", c"0.022", cvar::ARCHIVE)
                .unwrap(),
            m_yaw: engine
                .create_cvar(c"m_yaw", c"0.022", cvar::ARCHIVE)
                .unwrap(),
            m_forward: engine
                .create_cvar(c"m_forward", c"1", cvar::ARCHIVE)
                .unwrap(),
            m_side: engine
                .create_cvar(c"m_side", c"0.8", cvar::ARCHIVE)
                .unwrap(),

            m_rawinput: engine
                .create_cvar(c"m_rawinput", c"1", cvar::ARCHIVE)
                .unwrap(),
            m_filter: engine
                .create_cvar(c"m_filter", c"0", cvar::ARCHIVE)
                .unwrap(),
            sensitivity: engine
                .create_cvar(c"sensitivity", c"3", cvar::ARCHIVE | cvar::FILTERABLE)
                .unwrap(),

            m_customaccel: engine
                .create_cvar(c"m_customaccel", c"0", cvar::ARCHIVE)
                .unwrap(),
            m_customaccel_scale: engine
                .create_cvar(c"m_customaccel_scale", c"0.04", cvar::ARCHIVE)
                .unwrap(),
            m_customaccel_max: engine
                .create_cvar(c"m_customaccel_max", c"0", cvar::ARCHIVE)
                .unwrap(),
            m_customaccel_exponent: engine
                .create_cvar(c"m_customaccel_exponent", c"1", cvar::ARCHIVE)
                .unwrap(),
        }
    }

    pub fn clear_states(&mut self) {
        if !self.mouse_active {
            return;
        }
        self.mx_accum = 0;
        self.my_accum = 0;
        self.mouse_oldbuttonstate = 0;
    }

    fn use_raw_input(&self) -> bool {
        self.m_rawinput.get()
    }

    pub fn activate_mouse(&mut self) {
        if self.mouse_initialized {
            self.mouse_active = true;
        }
    }

    pub fn deactivate_mouse(&mut self) {
        if self.mouse_initialized {
            self.mouse_active = false;
        }
    }

    pub fn get_mouse_sensitivity(&self) -> f32 {
        let v = self.sensitivity.get();
        if !(0.01..=10000.0).contains(&v) {
            let v = v.clamp(0.01, 10000.0);
            self.sensitivity.set(v);
            v
        } else {
            v
        }
    }

    pub fn in_mlook_state(&self) -> KeyState {
        self.in_mlook.state()
    }

    pub fn button_bits(&self, reset_state: bool, show_score: bool) -> c_int {
        let mut bits = 0;

        macro_rules! set {
            ($($name:expr => $bits:expr),* $(,)?) => (
                $(if $name.state().intersects(KeyState::ANY_DOWN) {
                    bits |= $bits;
                })*
            );
        }

        set! {
            self.in_attack => consts::IN_ATTACK,
            self.in_duck => consts::IN_DUCK,
            self.in_jump => consts::IN_JUMP,
            self.in_forward => consts::IN_FORWARD,
            self.in_back => consts::IN_BACK,
            self.in_use => consts::IN_USE,
            self.in_left => consts::IN_LEFT,
            self.in_right => consts::IN_RIGHT,
            self.in_moveleft => consts::IN_MOVELEFT,
            self.in_moveright => consts::IN_MOVERIGHT,
            self.in_attack2 => consts::IN_ATTACK2,
            self.in_reload => consts::IN_RELOAD,
            self.in_alt1 => consts::IN_ALT1,
            self.in_score => consts::IN_SCORE,
        }

        if self.in_cancel.get() {
            bits |= consts::IN_CANCEL;
        }

        if show_score {
            bits |= consts::IN_SCORE;
        }

        if reset_state {
            macro_rules! reset {
                ($($name:expr),* $(,)?) => (
                    $($name.with_state(|f| f.difference(KeyState::IMPULSE_DOWN));)*
                );
            }
            reset! {
                self.in_attack,
                self.in_duck,
                self.in_jump,
                self.in_forward,
                self.in_back,
                self.in_use,
                self.in_left,
                self.in_right,
                self.in_moveleft,
                self.in_moveright,
                self.in_attack2,
                self.in_reload,
                self.in_alt1,
                self.in_score,
            }
        }

        bits
    }

    pub fn reset_button_bits(&self, bits: c_int, show_score: bool) {
        let bits_new = self.button_bits(false, show_score) ^ bits;

        if bits_new & consts::IN_ATTACK != 0 {
            if bits & consts::IN_ATTACK != 0 {
                self.in_attack.key_down();
            } else {
                self.in_attack.set_state(KeyState::NONE);
            }
        }
    }

    pub fn in_commands(&self) {
        // TODO: joystick
    }

    pub fn mouse_in_use(&self, value: bool) {
        self.mouse_in_use.set(value);
    }

    fn get_relative_mouse_pos(&mut self) -> (c_int, c_int) {
        if !self.use_raw_input() {
            let engine = self.engine;
            let screen = engine.screen_info();
            let (cx, cy) = (screen.width() / 2, screen.height() / 2);
            let (mx, my) = if !self.mouse_raw_used {
                engine.get_mouse_position()
            } else {
                // If raw input used we need to reset cursor position before we can
                // get valid relative position.
                self.mouse_raw_used = false;
                (cx, cy)
            };
            engine.set_mouse_position(cx, cy);
            (mx - cx, my - cy)
        } else {
            // Do not link SDL2 for tests.
            #[cfg(not(test))]
            {
                let mut dx = 0;
                let mut dy = 0;
                #[link(name = "SDL2-2.0")]
                unsafe extern "C" {
                    fn SDL_GetRelativeMouseState(x: &mut c_int, y: &mut c_int) -> u32;
                }
                unsafe {
                    SDL_GetRelativeMouseState(&mut dx, &mut dy);
                }
                if !self.mouse_raw_used {
                    self.mouse_raw_used = true;
                }
                (dx, dy)
            }

            #[cfg(test)]
            (0, 0)
        }
    }

    pub fn accumulate(&mut self) {
        if !self.mouse_in_use.get() && !self.mouse_visible && self.mouse_active {
            let (dx, dy) = self.get_relative_mouse_pos();
            self.mx_accum += dx;
            self.my_accum += dy;
        }
    }

    fn scale_mouse(&mut self, hud: &Hud) {
        let mx = self.mouse_x;
        let my = self.mouse_y;

        let mut mouse_senstivity = hud.get_sensitivity();
        if mouse_senstivity == 0.0 {
            mouse_senstivity = self.get_mouse_sensitivity()
        }

        if self.m_customaccel.get() != 0.0 {
            let raw_mouse_movement_distance = sqrt((mx * mx + my * my) as f64);
            let acceleration_scale = self.m_customaccel_scale.get();
            let accelerated_sensitivity_max = self.m_customaccel_max.get();
            let accelerated_sensitivity_exponent = self.m_customaccel_exponent.get();
            let mut accelerated_sensitivity = (pow(
                raw_mouse_movement_distance,
                accelerated_sensitivity_exponent,
            ) * acceleration_scale
                + mouse_senstivity as f64) as f32;

            if accelerated_sensitivity_max > 0.0001
                && accelerated_sensitivity > accelerated_sensitivity_max
            {
                accelerated_sensitivity = accelerated_sensitivity_max;
            }

            self.mouse_x *= accelerated_sensitivity;
            self.mouse_y *= accelerated_sensitivity;

            if self.m_customaccel.get() == 2.0 {
                self.mouse_x *= self.m_yaw.get();
                self.mouse_y *= self.m_pitch.get();
            }
        } else {
            self.mouse_x *= mouse_senstivity;
            self.mouse_y *= mouse_senstivity;
        }
    }

    fn mouse_move(&mut self, hud: &Hud, view: &View, _frametime: f32, cmd: &mut usercmd_s) {
        let engine = self.engine;
        let mut viewangles = engine.get_view_angles();

        let in_mlook_state = self.in_mlook.state();
        if in_mlook_state.contains(KeyState::DOWN) {
            view.stop_pitch_drift();
        }

        if !self.mouse_in_use.get() && !hud.state.intermission() && !self.mouse_visible {
            let (dx, dy) = self.get_relative_mouse_pos();

            let mx = dx + self.mx_accum;
            let my = dy + self.my_accum;

            self.mx_accum = 0;
            self.my_accum = 0;

            if self.m_filter.get() {
                self.mouse_x = (mx + self.old_mouse_x) as f32 * 0.5;
                self.mouse_y = (my + self.old_mouse_y) as f32 * 0.5;
            } else {
                self.mouse_x = mx as f32;
                self.mouse_y = my as f32;
            }

            self.old_mouse_x = mx;
            self.old_mouse_y = my;

            self.scale_mouse(hud);

            if self.in_strafe.is_down()
                || self.lookstrafe.get() && in_mlook_state.contains(KeyState::DOWN)
            {
                cmd.sidemove += self.m_side.get() * self.mouse_x;
            } else {
                viewangles[YAW] -= self.m_yaw.get() * self.mouse_x;
            }

            if in_mlook_state.contains(KeyState::DOWN) && !self.in_strafe.is_down() {
                viewangles[PITCH] += self.m_pitch.get() * self.mouse_y;
                if viewangles[PITCH] > self.cl_pitchdown.get() {
                    viewangles[PITCH] = self.cl_pitchdown.get();
                }
                if viewangles[PITCH] < -self.cl_pitchup.get() {
                    viewangles[PITCH] = -self.cl_pitchup.get();
                }
            } else if self.in_strafe.is_down() && engine.is_no_clipping() {
                cmd.upmove -= self.m_forward.get() * self.mouse_y;
            } else {
                cmd.forwardmove -= self.m_forward.get() * self.mouse_y;
            }
        }

        engine.set_view_angles(viewangles);
    }

    fn adjust_angles(&self, view: &View, frametime: f32, viewangles: &mut vec3_t) {
        let speed = if self.in_speed.is_down() {
            frametime * self.cl_anglespeedkey.get()
        } else {
            frametime
        };

        if !self.in_strafe.is_down() {
            let yawspeed = self.cl_yawspeed.get();
            viewangles[YAW] -= speed * yawspeed * self.in_right.key_state();
            viewangles[YAW] += speed * yawspeed * self.in_left.key_state();
            viewangles[YAW] = angle_mod(viewangles[YAW]);
        }

        let pitchspeed = self.cl_pitchspeed.get();
        if self.in_klook.is_down() {
            view.stop_pitch_drift();
            viewangles[PITCH] -= speed * pitchspeed * self.in_forward.key_state();
            viewangles[PITCH] += speed * pitchspeed * self.in_back.key_state();
        }

        let up = self.in_lookup.key_state();
        let down = self.in_lookdown.key_state();

        viewangles[PITCH] -= speed * pitchspeed * up;
        viewangles[PITCH] += speed * pitchspeed * down;

        if up != 0.0 || down != 0.0 {
            view.stop_pitch_drift();
        }

        let pitchdown = self.cl_pitchdown.get();
        let pitchup = self.cl_pitchup.get();
        viewangles[PITCH] = viewangles[PITCH].clamp(-pitchup, pitchdown);

        viewangles[ROLL] = viewangles[ROLL].clamp(-50.0, 50.0);
    }

    pub fn mouse_event(&mut self, mstate: c_int) {
        if self.mouse_in_use.get() || self.mouse_visible {
            return;
        }

        let engine = self.engine;
        for i in 0..self.mouse_buttons {
            if mstate & (1 << i) != 0 && self.mouse_oldbuttonstate & (1 << i) == 0 {
                engine.key_event(keys::K_MOUSE1 + i, true);
            }

            if mstate & (1 << i) == 0 && self.mouse_oldbuttonstate & (1 << i) != 0 {
                engine.key_event(keys::K_MOUSE1 + i, false);
            }
        }

        self.mouse_oldbuttonstate = mstate;
    }

    fn in_move(&mut self, hud: &Hud, view: &View, frametime: f32, cmd: &mut usercmd_s) {
        if !self.mouse_in_use.get() && self.mouse_active {
            self.mouse_move(hud, view, frametime, cmd);
        }

        // TODO: joystick
    }

    pub fn create_move(
        &mut self,
        hud: &Hud,
        view: &View,
        frametime: f32,
        active: bool,
    ) -> usercmd_s {
        let engine = self.engine;
        let mut cmd: usercmd_s = unsafe { mem::zeroed() };

        if active {
            let mut viewangles = engine.get_view_angles();
            self.adjust_angles(view, frametime, &mut viewangles);
            engine.set_view_angles(viewangles);

            if self.in_strafe.is_down() {
                let sidespeed = self.cl_sidespeed.get();
                cmd.sidemove += sidespeed * self.in_right.key_state();
                cmd.sidemove -= sidespeed * self.in_left.key_state();
            }

            let sidespeed = self.cl_sidespeed.get();
            cmd.sidemove += sidespeed * self.in_moveright.key_state();
            cmd.sidemove -= sidespeed * self.in_moveleft.key_state();

            let upspeed = self.cl_upspeed.get();
            cmd.upmove += upspeed * self.in_up.key_state();
            cmd.upmove -= upspeed * self.in_down.key_state();

            if !self.in_klook.is_down() {
                let forwardspeed = self.cl_forwardspeed.get();
                let backspeed = self.cl_backspeed.get();
                cmd.forwardmove += forwardspeed * self.in_forward.key_state();
                cmd.forwardmove -= backspeed * self.in_back.key_state();
            }

            if self.in_speed.is_down() {
                let cl_movespeedkey = self.cl_movespeedkey.get();
                cmd.forwardmove *= cl_movespeedkey;
                cmd.sidemove *= cl_movespeedkey;
                cmd.upmove *= cl_movespeedkey;
            }

            let spd = engine.get_client_max_speed();
            if spd != 0.0 {
                let fmov = sqrtf(
                    (cmd.forwardmove * cmd.forwardmove)
                        + (cmd.sidemove * cmd.sidemove)
                        + (cmd.upmove * cmd.upmove),
                );

                if fmov > spd {
                    let r = spd / fmov;
                    cmd.forwardmove *= r;
                    cmd.sidemove *= r;
                    cmd.upmove *= r;
                }
            }

            self.in_move(hud, view, frametime, &mut cmd);
        }

        cmd.impulse = self.in_impulse.take();
        cmd.weaponselect = hud.take_weapon_select() as u8;
        cmd.buttons = self.button_bits(true, hud.show_score()) as u16;

        let viewangles = engine.get_view_angles();
        if unsafe { helpers::g_iAlive != 0 } {
            cmd.viewangles = viewangles;
            self.oldangles = viewangles;
        } else {
            cmd.viewangles = self.oldangles;
        }

        cmd
    }
}
