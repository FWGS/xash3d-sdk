mod inventory;

mod ammo;
mod battery;
mod death_notice;
mod flashlight;
mod geiger;
mod health;
mod history;
mod menu;
mod message;
mod say_text;
mod scoreboard;
mod text_message;
mod train;
pub mod weapon_menu;

use core::{
    any::{Any, TypeId},
    cell::{Cell, Ref, RefCell, RefMut},
    cmp,
    ffi::c_int,
    fmt::Write,
};

use alloc::{rc::Rc, string::String, vec::Vec};
use bitflags::bitflags;
use xash3d_client::{
    color::RGB,
    consts::MAX_PLAYERS,
    csz::{CStrArray, CStrBox, CStrThin},
    cvar::{self, Cvar},
    ffi::{client::client_data_s, common::vec3_t},
    macros::hook_command,
    prelude::*,
    sprite::{DigitSprites, Sprite, SpriteHandle, Sprites},
    user_message::hook_user_message,
};
use xash3d_hl_shared::{user_message, weapons::Weapons};

use crate::{
    export::{hud, input},
    hud::{
        health::Health, inventory::Inventory, menu::Menu, scoreboard::ScoreBoard,
        text_message::TextMessage, weapon_menu::WeaponMenu,
    },
};

pub const MAX_WEAPONS: usize = 32;

const MIN_ALPHA: u8 = 100;

const FADE_TIME: f32 = 100.0;
const FADE_TIME_AMMO: f32 = 200.0;

// const DEFAULT_COLOR: RGB = RGB::YELLOWISH;
const DEFAULT_COLOR: RGB = RGB::new(255, 0, 255); // TODO: remove me

const MAX_PLAYER_NAME_LENGTH: usize = 32;

fn lower_sprite_resolution(res: u32) -> u32 {
    match res {
        2560 => 1280,
        1280 => 640,
        640 => 320,
        320 => 320,
        _ => {
            warn!("unexpected sprite resolution {res}");
            320
        }
    }
}

fn try_spr_load<F>(init: u32, load: F) -> Option<SpriteHandle>
where
    F: Fn(u32) -> Option<SpriteHandle>,
{
    let mut res = init;
    loop {
        match load(res) {
            Some(hspr) => return Some(hspr),
            None => {
                let lower = lower_sprite_resolution(res);
                if res == lower {
                    // lowest resolution
                    return None;
                }
                // try with lower resolution
                res = lower;
            }
        }
    }
}

#[derive(Copy, Clone)]
struct Fade {
    value: f32,
    time: f32,
}

impl Default for Fade {
    fn default() -> Self {
        Self::new(FADE_TIME)
    }
}

impl Fade {
    fn new(time: f32) -> Self {
        Self { value: 0.0, time }
    }

    fn start(&mut self) {
        self.value = self.time;
    }

    fn stop(&mut self) {
        self.value = 0.0;
    }

    fn alpha(&mut self, delta: f64) -> u8 {
        let mut a = MIN_ALPHA;
        if self.value != 0.0 {
            if self.value > self.time {
                self.value = self.time;
            }
            self.value -= (delta * 20.0) as f32;
            if self.value <= 0.0 {
                self.value = 0.0;
            }
            a += (self.value / self.time * 128.0) as u8;
        }
        a
    }
}

bitflags! {
    #[derive(Copy, Clone)]
    #[repr(transparent)]
    pub struct HudFlags: u32 {
        const NONE = 0;
        const ACTIVE = 1;
        const INTERMISSION = 2;
    }
}

#[allow(unused_variables)]
pub trait HudItem: Any {
    fn flags(&self) -> HudFlags {
        HudFlags::ACTIVE
    }

    fn vid_init(&mut self, state: &State) {}

    fn init_hud_data(&mut self, state: &State) {}

    fn reset(&mut self) {}

    fn think(&mut self, state: &State) {}

    fn draw(&mut self, state: &State) {}
}

bitflags! {
    /// Flags to hide HUD items.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct Hide: u32 {
        /// Hide weapons.
        const WEAPONS       = 1 << 0;
        /// Hide flashlight.
        const FLASHLIGHT    = 1 << 1;
        /// Hide all.
        const ALL           = 1 << 2;
        /// Hide health.
        const HEALTH        = 1 << 3;
    }
}

#[derive(Copy, Clone, Default)]
pub struct PlayerInfoExtra {
    pub frags: i16,
    pub deaths: i16,
    pub teamnumber: i16,
}

pub struct State {
    engine: ClientEngineRef,

    /// Is the game in an intermission/pause?
    intermission: Cell<bool>,
    /// Previous engine time.
    time_old: Cell<f32>,
    /// Current engine time.
    time: Cell<f32>,
    /// The difference between current and previous engine time.
    time_delta: Cell<f64>,

    /// Default color for HUD items.
    color: Cell<RGB>,
    /// Current hide flags for HUD items.
    hide: Cell<Hide>,

    /// The position of player in the world.
    origin: Cell<vec3_t>,
    /// The camera angles.
    angles: Cell<vec3_t>,

    last_fov: Cell<u8>,
    fov: Cell<u8>,
    mouse_sensitivity: Cell<f32>,
    key_bits: Cell<u32>,

    /// Player inventory.
    inv: RefCell<Inventory>,

    /// Default sprite resolution to load.
    sprite_resolution: Cell<u32>,
    /// HUD sprites.
    sprites: RefCell<Sprites>,
    /// HUD sprites for numbers.
    digits: RefCell<DigitSprites>,

    server_name: RefCell<CStrBox>,
    player_info_extra: RefCell<[Option<PlayerInfoExtra>; MAX_PLAYERS + 1]>,
}

impl State {
    fn new(engine: ClientEngineRef) -> Self {
        Self {
            engine,
            intermission: Cell::default(),
            time_old: Cell::default(),
            time: Cell::new(1.0),
            time_delta: Cell::default(),
            color: Cell::new(DEFAULT_COLOR),
            hide: Cell::default(),
            origin: Cell::default(),
            angles: Cell::default(),
            last_fov: Cell::default(),
            fov: Cell::default(),
            mouse_sensitivity: Cell::default(),
            key_bits: Cell::default(),
            inv: RefCell::new(Inventory::new(engine)),
            sprite_resolution: Cell::default(),
            sprites: RefCell::new(Sprites::new(engine)),
            digits: RefCell::new(DigitSprites::new()),
            server_name: RefCell::default(),
            player_info_extra: RefCell::new([None; MAX_PLAYERS + 1]),
        }
    }

    fn vid_init(&self) {
        let engine = self.engine;
        let screen_info = engine.screen_info();
        let res = screen_info.sprite_resolution();
        self.sprite_resolution.set(res);
        let mut sprites = self.sprites.borrow_mut();
        sprites.reload_from_file(res, c"sprites/hud.txt");
        self.digits.replace(DigitSprites::from_sprites(&sprites));

        let mut inv = self.inv.borrow_mut();
        inv.vid_init();
        if !engine.is_spectator_only() && !self.is_hidden(Hide::WEAPONS | Hide::ALL) {
            inv.set_crosshair();
        }
    }

    fn think(&self) {
        self.inv.borrow_mut().think();
    }

    pub fn intermission(&self) -> bool {
        self.intermission.get()
    }

    pub fn time(&self) -> f32 {
        self.time.get()
    }

    pub fn time_delta(&self) -> f64 {
        self.time_delta.get()
    }

    pub fn color(&self) -> RGB {
        self.color.get()
    }

    pub fn set_color(&self, color: RGB) {
        self.color.set(color)
    }

    pub fn hide(&self) -> Hide {
        self.hide.get()
    }

    pub fn set_hide(&self, hide: Hide) {
        self.hide.set(hide);
    }

    pub fn is_hidden(&self, value: Hide) -> bool {
        self.hide().intersects(value)
    }

    pub fn origin(&self) -> vec3_t {
        self.origin.get()
    }

    pub fn angles(&self) -> vec3_t {
        self.angles.get()
    }

    pub fn fov(&self) -> u8 {
        self.fov.get()
    }

    pub fn key_bits(&self) -> u32 {
        self.key_bits.get()
    }

    pub fn set_key_bits(&self, bits: u32) {
        self.key_bits.set(bits);
    }

    pub fn with_key_bits(&self, map: impl FnOnce(u32) -> u32) {
        self.set_key_bits(map(self.key_bits()));
    }

    pub fn inventory(&self) -> Ref<'_, Inventory> {
        self.inv.borrow()
    }

    pub fn inventory_mut(&self) -> RefMut<'_, Inventory> {
        self.inv.borrow_mut()
    }

    fn has_suit(&self) -> bool {
        self.inventory().weapons().contains(Weapons::SUIT)
    }

    pub fn sprite_resolution(&self) -> u32 {
        self.sprite_resolution.get()
    }

    fn find_sprite_index(&self, name: impl AsRef<CStrThin>) -> Option<usize> {
        self.sprites.borrow().find_index(name)
    }

    fn find_sprite(&self, name: impl AsRef<CStrThin>) -> Option<Sprite> {
        self.sprites.borrow().find(name).copied()
    }

    fn sprite(&self, index: usize) -> Option<Sprite> {
        self.sprites.borrow().get(index).map(|i| **i)
    }

    fn sprites(&self) -> Ref<'_, Sprites> {
        self.sprites.borrow()
    }

    fn digits(&self) -> Ref<'_, DigitSprites> {
        self.digits.borrow()
    }

    fn draw_number(&self, number: c_int) -> DrawNumber<'_> {
        DrawNumber {
            engine: self.engine,
            state: self,
            width: 0,
            color: self.color.get(),
            number,
            zero: true,
            string: false,
            reverse: false,
        }
    }

    fn server_name(&self) -> Ref<'_, CStrThin> {
        Ref::map(self.server_name.borrow(), |s| s.as_c_str())
    }

    fn set_server_name(&self, name: impl Into<CStrBox>) {
        self.server_name.replace(name.into());
    }

    fn set_player_info_extra(&self, index: usize, info: Option<PlayerInfoExtra>) {
        self.player_info_extra.borrow_mut()[index] = info;
    }

    fn get_client_color(&self, _client: c_int) -> RGB {
        const BLUE: RGB = RGB::new(153, 204, 255);
        const RED: RGB = RGB::new(255, 64, 64);
        const GREEN: RGB = RGB::new(153, 255, 153);
        const YELLOW: RGB = RGB::new(255, 178, 0);
        const GREY: RGB = RGB::new(204, 204, 204);

        let teamnumber = 0; // TODO: extra player info
        match teamnumber {
            0 => YELLOW,
            1 => BLUE,
            2 => RED,
            3 => YELLOW,
            4 => GREEN,
            _ => GREY,
        }
    }
}

#[must_use]
struct DrawNumber<'a> {
    engine: ClientEngineRef,
    state: &'a State,
    width: usize,
    number: c_int,
    color: RGB,
    zero: bool,
    string: bool,
    reverse: bool,
}

impl DrawNumber<'_> {
    fn width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    // fn no_zero(mut self) -> Self {
    //     self.zero = false;
    //     self
    // }

    fn color(mut self, color: RGB) -> Self {
        self.color = color;
        self
    }

    fn string(mut self, string: bool) -> Self {
        self.string = string;
        self
    }

    fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    fn at(self, x: c_int, y: c_int) -> c_int {
        let digits = self.state.digits();
        if self.number == 0 && !self.zero {
            return x + digits.width() * self.width as c_int;
        }

        let mut buf = CStrArray::<64>::new();
        write!(buf.cursor(), "{:1$}", self.number, self.width).ok();

        let engine = self.engine;
        if self.string {
            if self.reverse {
                return engine.draw_string_reverse(x, y, buf.as_c_str(), self.color);
            } else {
                return engine.draw_string(x, y, buf.as_c_str(), self.color);
            }
        }

        if self.reverse {
            todo!("draw number reverse is not implemented for sprites");
        }

        let mut x = x;
        for c in buf.bytes() {
            if let Some(digit) = digits.get_by_char(c as char) {
                digit.draw_additive(0, x, y, self.color);
            }
            x += digits.width();
        }
        x
    }
}

type RcCell<T> = Rc<RefCell<T>>;

#[derive(Default)]
pub struct Items {
    items: Vec<(TypeId, RcCell<dyn HudItem>)>,
}

impl Items {
    pub fn add<T: HudItem + 'static>(&mut self, value: T) -> &mut Self {
        let id = value.type_id();
        let item = Rc::new(RefCell::new(value));
        match self.items.binary_search_by_key(&id, |(id, _)| *id) {
            Ok(index) => self.items[index].1 = item,
            Err(index) => self.items.insert(index, (id, item)),
        }
        self
    }

    pub fn find<T: Any>(&self) -> Option<&RcCell<T>> {
        let id = TypeId::of::<T>();
        match self.items.binary_search_by_key(&id, |(i, _)| *i) {
            Ok(i) => {
                let item = &self.items[i].1 as *const RcCell<dyn HudItem>;
                Some(unsafe { &*(item as *const RcCell<T>) })
            }
            Err(_) => None,
        }
    }

    pub fn get<T: Any>(&self) -> Ref<'_, T> {
        self.find::<T>().unwrap().borrow()
    }

    pub fn get_mut<T: Any>(&self) -> RefMut<'_, T> {
        self.find::<T>().unwrap().borrow_mut()
    }

    fn iter(&self) -> impl Iterator<Item = RefMut<'_, dyn HudItem>> {
        self.items.iter().map(|(_, i)| i.borrow_mut())
    }
}

pub struct Hud {
    engine: ClientEngineRef,

    pub state: State,
    pub items: Items,

    #[allow(dead_code)]
    pub text_message: TextMessage,

    logo: Cell<bool>,
    logo_hspr: Cell<Option<SpriteHandle>>,
    old_hud_color: RefCell<String>,

    zoom_sensitivity_ratio: Cvar,
    default_fov: Cvar<u8>,
    hud_draw: Cvar<bool>,
    hud_color: Cvar<CStrThin>,
}

impl Hud {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_messages_and_commands(engine);

        let mut items = Items::default();
        items
            .add(ammo::Ammo::new(engine))
            .add(history::History::new(engine))
            .add(weapon_menu::WeaponMenu::new(engine))
            .add(health::Health::new(engine))
            .add(battery::Battery::new(engine))
            .add(flashlight::Flashlight::new(engine))
            .add(geiger::Geiger::new(engine))
            .add(train::Train::new(engine))
            .add(death_notice::DeathNotice::new(engine))
            .add(say_text::SayText::new(engine))
            .add(menu::Menu::new(engine))
            .add(message::HudMessage::new(engine))
            .add(scoreboard::ScoreBoard::new(engine));

        engine.register_cvar(c"cl_autowepswitch", c"1", cvar::ARCHIVE | cvar::USER_INFO);

        Self {
            engine,

            state: State::new(engine),
            items,

            text_message: TextMessage::new(engine),

            logo: Cell::default(),
            logo_hspr: Cell::default(),
            old_hud_color: RefCell::default(),

            zoom_sensitivity_ratio: engine
                .create_cvar(c"zoom_sensitivity_ratio", c"1.2", cvar::ARCHIVE)
                .unwrap(),
            default_fov: engine
                .create_cvar(c"default_fov", c"90", cvar::ARCHIVE)
                .unwrap(),
            hud_draw: engine
                .create_cvar(c"hud_draw", c"1", cvar::ARCHIVE)
                .unwrap(),
            hud_color: engine
                .create_cvar(c"hud_color", c"", cvar::ARCHIVE)
                .unwrap(),
        }
    }

    pub fn get_sensitivity(&self) -> f32 {
        self.state.mouse_sensitivity.get()
    }

    pub fn set_fov(&self, fov: u8) {
        let default_fov = self.default_fov.get();
        self.state.fov.set(if fov == 0 { default_fov } else { fov });

        self.state
            .mouse_sensitivity
            .set(if self.state.fov.get() != default_fov {
                let zsr = self.zoom_sensitivity_ratio.get();
                input().get_mouse_sensitivity() * (fov as f32 / default_fov as f32) * zsr
            } else {
                0.0
            });
    }

    pub fn last_fov(&self) -> u8 {
        // TODO: demo api
        self.state.last_fov.get()
    }

    pub fn set_last_fov(&self, fov: u8) {
        self.state.last_fov.set(fov);
    }

    pub fn fov(&self) -> u8 {
        self.state.fov.get()
    }

    pub fn score_info(&self, info: &user_message::ScoreInfo) {
        let cl = info.cl;
        if cl > 0 && cl <= MAX_PLAYERS as u8 {
            let index = cl as usize;
            let extra = PlayerInfoExtra {
                frags: info.frags,
                deaths: info.deaths,
                teamnumber: cmp::max(0, info.teamnumber),
            };
            self.items.get_mut::<ScoreBoard>().score_info(cl, &extra);
            self.state.set_player_info_extra(index, Some(extra));
        }
    }

    pub fn show_score_board(&self, show: bool) {
        if self.engine.is_multiplayer() {
            self.items.get_mut::<ScoreBoard>().show(show);
        }
    }

    pub fn vid_init(&self) {
        self.logo_hspr.set(None);

        self.state.vid_init();

        for mut i in self.items.iter() {
            i.vid_init(&self.state);
        }
    }

    fn init_hud(&self) {
        for mut i in self.items.iter() {
            i.init_hud_data(&self.state);
        }

        self.reset();
    }

    pub fn reset(&self) {
        self.state.inv.borrow_mut().reset();
        self.state.fov.set(90);
        self.state.last_fov.set(self.default_fov.get());

        for mut i in self.items.iter() {
            i.reset();
        }
    }

    pub fn take_weapon_select(&self) -> u32 {
        self.items.get_mut::<WeaponMenu>().take_weapon_select()
    }

    fn think(&self) {
        self.state.think();

        for mut i in self.items.iter() {
            i.think(&self.state);
        }

        self.set_fov(self.last_fov());
        if self.state.fov.get() == 0 {
            self.state.fov.set(cmp::max(90, self.default_fov.get()));
        }
    }

    pub fn show_score(&self) -> bool {
        self.items.get::<Health>().is_dead() || self.state.intermission()
    }

    pub fn update_client_data(&self, data: &mut client_data_s, _time: f32) -> bool {
        self.state.origin.set(data.origin);
        self.state.angles.set(data.viewangles);

        let show_score = self.show_score();
        self.state
            .set_key_bits(input().button_bits(false, show_score));

        self.state
            .inventory_mut()
            .weapons_set(Weapons::from_bits_retain(data.iWeaponBits as u32));

        self.think();

        data.fov = self.fov() as f32;

        input().reset_button_bits(self.state.key_bits(), show_score);

        // data has been changed
        true
    }

    fn update_hud_color(&self) {
        const COLOR_MAP: [(&str, RGB); 16] = [
            ("black", RGB::BLACK),
            ("silver", RGB::SILVER),
            ("gray", RGB::GRAY),
            ("white", RGB::WHITE),
            ("maroon", RGB::MAROON),
            ("red", RGB::RED),
            ("green", RGB::GREEN),
            ("lime", RGB::LIME),
            ("navy", RGB::NAVY),
            ("blue", RGB::BLUE),
            ("yellowish", RGB::YELLOWISH),
            ("redish", RGB::REDISH),
            ("greenish", RGB::GREENISH),
            ("purple", RGB::PURPLE),
            ("fuchsia", RGB::FUCHSIA),
            ("cyan", RGB::CYAN),
        ];

        let s = self.hud_color.get();
        let Ok(s) = s.to_str() else { return };

        let mut old_hud_color = self.old_hud_color.borrow_mut();
        if old_hud_color.as_str() == s {
            return;
        }
        old_hud_color.clear();
        old_hud_color.push_str(s);

        if s.is_empty() {
            self.state.set_color(DEFAULT_COLOR);
            return;
        }

        if s == "help" {
            info!("  empty (default color), hex value (RGB, RRGGBB) or color name:");
            for (color, _) in &COLOR_MAP {
                info!("    {color}");
            }
            return;
        }

        let color = match COLOR_MAP.iter().find(|i| i.0 == s) {
            Some((_, color)) => *color,
            None => parse_color(s).unwrap_or_else(|| {
                warn!("invalid hud_color {s:?}");
                DEFAULT_COLOR
            }),
        };
        self.state.set_color(color);
    }

    fn draw_logo(&self) {
        const MAX_LOGO_FRAMES: usize = 56;
        const LOGO_FRAME: [c_int; MAX_LOGO_FRAMES] = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 13, 13, 13, 13, 13, 12, 11, 10, 9, 8, 14,
            15, 16, 17, 18, 19, 20, 20, 20, 20, 20, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 29, 29,
            29, 29, 29, 28, 27, 26, 25, 24, 30, 31,
        ];

        if !self.logo.get() {
            return;
        }

        let engine = self.engine;
        if self.logo_hspr.get().is_none() {
            self.logo_hspr
                .set(try_spr_load(self.state.sprite_resolution(), |res| {
                    engine.spr_load(format_args!("sprites/{res}_logo.spr"))
                }));
        }

        let Some(hspr) = self.logo_hspr.get() else {
            return;
        };
        let info = engine.screen_info();
        let (w, h) = hspr.size(0);
        let x = info.width() - w;
        let y = h / 2;
        let frame = (self.state.time() * 20.0) as usize % MAX_LOGO_FRAMES;
        let i = LOGO_FRAME[frame] - 1;
        hspr.draw_additive(i, x, y, RGB::splat(250));
    }

    pub fn draw(&self, time: f32, intermission: bool) -> bool {
        let time_old = self.state.time.replace(time);
        self.state.time_old.set(time_old);
        let mut time_delta = time as f64 - time_old as f64;
        if time_delta < 0.0 {
            time_delta = 0.0;
        }
        self.state.time_delta.set(time_delta);

        self.state.intermission.set(intermission);

        if self.hud_draw.get() {
            self.update_hud_color();
            for mut i in self.items.iter() {
                let flags = i.flags();
                if !intermission {
                    if flags.contains(HudFlags::ACTIVE) && !self.state.is_hidden(Hide::ALL) {
                        i.draw(&self.state);
                    }
                } else if flags.contains(HudFlags::INTERMISSION) {
                    i.draw(&self.state);
                }
            }
        }

        self.draw_logo();
        true
    }
}

fn hex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

fn parse_color(s: &str) -> Option<RGB> {
    if !s
        .chars()
        .all(|i| i.is_ascii_digit() || ('a'..='f').contains(&i) || ('A'..='F').contains(&i))
    {
        return None;
    }
    let s = s.as_bytes();
    if ![3, 6].contains(&s.len()) {
        return None;
    }
    let mut c = [0; 3];
    if s.len() == 3 {
        for i in 0..3 {
            let x = hex(s[i]);
            c[i] = x | (x << 4);
        }
    } else {
        for (i, c) in c.iter_mut().enumerate() {
            let j = i * 2;
            *c = (hex(s[j]) << 4) | hex(s[j]);
        }
    }
    Some(c.into())
}

fn hook_messages_and_commands(engine: ClientEngineRef) {
    hook_user_message!(engine, Logo, |_, msg| {
        let value = msg.read_u8()?;
        hud().logo.set(value != 0);
        Ok(())
    });

    hook_user_message!(engine, InitHUD, {
        hud().init_hud();
        true
    });

    hook_user_message!(engine, GameMode, {
        trace!("message GameMode is not implemented");
        true
    });

    hook_user_message!(engine, SetFOV, |_, msg| {
        let msg = msg.read::<user_message::SetFOV>()?;
        let hud = hud();
        hud.set_last_fov(msg.fov);
        hud.set_fov(msg.fov);
        Ok(())
    });

    hook_user_message!(engine, ScoreInfo, |_, msg| {
        let msg = msg.read::<user_message::ScoreInfo>()?;
        hud().score_info(&msg);
        Ok(())
    });

    hook_user_message!(engine, ServerName, |_, msg| {
        let msg = msg.read::<user_message::ServerName>()?;
        hud().state.set_server_name(msg.name);
        Ok(())
    });

    hook_user_message!(engine, ResetHUD, |_, msg| {
        let msg = msg.read::<user_message::ResetHUD>()?;
        if msg.x != 0 {
            warn!("ResetHUD: unexpected user message data");
        }
        hud().reset();
        Ok(())
    });

    fn cmd_slot(slot: u32) {
        let hud = hud();
        let mut menu = hud.items.get_mut::<Menu>();
        if menu.is_displayed() {
            menu.select_menu_item(slot);
        } else {
            hud.items
                .get_mut::<WeaponMenu>()
                .select_slot(&hud.state, slot);
        }
    }

    hook_command!(engine, c"slot1", |_| cmd_slot(1));
    hook_command!(engine, c"slot2", |_| cmd_slot(2));
    hook_command!(engine, c"slot3", |_| cmd_slot(3));
    hook_command!(engine, c"slot4", |_| cmd_slot(4));
    hook_command!(engine, c"slot5", |_| cmd_slot(5));
    hook_command!(engine, c"slot6", |_| cmd_slot(6));
    hook_command!(engine, c"slot7", |_| cmd_slot(7));
    hook_command!(engine, c"slot8", |_| cmd_slot(8));
    hook_command!(engine, c"slot9", |_| cmd_slot(9));
    hook_command!(engine, c"slot10", |_| cmd_slot(10));
}
