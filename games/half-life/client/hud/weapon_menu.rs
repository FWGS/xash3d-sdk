use core::{cmp, ffi::c_int, mem};

use xash3d_client::{
    color::RGB,
    consts,
    cvar::{self, Cvar},
    macros::hook_command,
    prelude::*,
};

use crate::{
    export::hud,
    hud::{
        Hide, Hud, HudItem, Sprite,
        inventory::{MAX_WEAPON_POSITIONS, MAX_WEAPON_SLOTS, Weapon},
    },
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Select {
    None,
    Menu,
    Weapon(u32, u32, u32),
}

impl From<&'_ Weapon> for Select {
    fn from(value: &'_ Weapon) -> Self {
        Self::Weapon(value.id, value.slot, value.slot_pos)
    }
}

pub struct WeaponMenu {
    engine: ClientEngineRef,
    active: Select,
    last: Select,
    bucket0: Option<usize>,
    selection: Option<Sprite>,
    ab_width: c_int,
    ab_height: c_int,
    weapon_select: u32,

    hud_fastswitch: Cvar<bool>,
}

impl WeaponMenu {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_command!(engine, c"cancelselect", |_| {
            hud().items.get_mut::<WeaponMenu>().cmd_close();
        });
        hook_command!(engine, c"invnext", |_| {
            let hud = hud();
            hud.items.get_mut::<WeaponMenu>().cmd_next_weapon(&hud);
        });
        hook_command!(engine, c"invprev", |_| {
            let hud = hud();
            hud.items.get_mut::<WeaponMenu>().cmd_prev_weapon(&hud);
        });

        Self {
            engine,
            active: Select::None,
            last: Select::None,
            bucket0: None,
            selection: None,
            weapon_select: 0,
            ab_width: 0,
            ab_height: 0,

            hud_fastswitch: engine
                .create_cvar(c"hud_fastswitch", c"0", cvar::ARCHIVE)
                .unwrap(),
        }
    }

    pub fn take_weapon_select(&mut self) -> u32 {
        mem::take(&mut self.weapon_select)
    }

    pub fn select_slot(&mut self, hud: &Hud, slot: u32) {
        if slot == 0 || slot > MAX_WEAPON_SLOTS as u32 {
            return;
        }

        if hud.is_hidden(Hide::WEAPONS | Hide::ALL) || !hud.has_suit() {
            return;
        }

        let engine = self.engine;
        let fast_switch = self.hud_fastswitch.get();

        let slot = slot - 1;
        let mut selected = None;

        let inv = hud.inventory();
        match self.active {
            Select::Weapon(_, s, p) if s == slot => {
                engine.play_sound_by_name(c"common/wpn_moveselect.wav", 1.0);

                if self.active != Select::None {
                    selected = inv.get_next_active_pos(s, p);
                }

                if selected.is_none() {
                    selected = inv.get_first_pos(slot);
                }
            }
            _ => {
                engine.play_sound_by_name(c"common/wpn_hudon.wav", 1.0);

                selected = inv.get_first_pos(slot);
                if let Some(weapon) = selected {
                    let next = inv.get_next_active_pos(weapon.slot, weapon.slot_pos);
                    if fast_switch && next.is_none() {
                        engine.server_cmd(&weapon.name);
                        self.weapon_select = weapon.id;
                        self.active = Select::None;
                        return;
                    }
                }
            }
        };

        self.active = match selected {
            Some(weapon) => weapon.into(),
            None if !fast_switch => Select::Menu,
            None => Select::None,
        };
    }

    pub fn close(&mut self) -> bool {
        if self.active != Select::None {
            self.last = self.active;
            self.active = Select::None;
            true
        } else {
            false
        }
    }

    fn cmd_close(&mut self) {
        if !self.close() {
            self.engine.client_cmd(c"escape");
        }
    }

    fn cmd_next_weapon(&mut self, hud: &Hud) {
        if hud.is_hidden(Hide::WEAPONS | Hide::ALL) {
            return;
        }

        let inv = hud.inventory();
        if matches!(self.active, Select::None | Select::Menu) {
            if let Some(weapon) = inv.current() {
                self.active = weapon.into();
            }
        }

        let mut slot = 0;
        let mut pos = 0;

        if let Select::Weapon(_, s, p) = self.active {
            slot = s;
            pos = p + 1;
        }

        for _ in 0..2 {
            for slot in slot..MAX_WEAPON_SLOTS as u32 {
                for pos in pos..MAX_WEAPON_POSITIONS as u32 {
                    if let Some(weapon) = inv.slot_weapon(slot, pos) {
                        if inv.has_ammo(weapon) {
                            self.active = weapon.into();
                            return;
                        }
                    }
                }
                pos = 0;
            }
            slot = 0;
        }

        self.active = Select::None;
    }

    fn cmd_prev_weapon(&mut self, hud: &Hud) {
        if hud.is_hidden(Hide::WEAPONS | Hide::ALL) {
            return;
        }

        let inv = hud.inventory();
        if matches!(self.active, Select::None | Select::Menu) {
            if let Some(weapon) = inv.current() {
                self.active = weapon.into();
            }
        }

        let mut slot = MAX_WEAPON_SLOTS as u32 - 1;
        let mut pos = MAX_WEAPON_POSITIONS as u32 - 1;

        if let Select::Weapon(_, s, p) = self.active {
            slot = s;
            pos = p;
        }

        for _ in 0..2 {
            for slot in (0..=slot).rev() {
                for pos in (0..pos).rev() {
                    if let Some(weapon) = inv.slot_weapon(slot, pos) {
                        if inv.has_ammo(weapon) {
                            self.active = weapon.into();
                            return;
                        }
                    }
                }
                pos = MAX_WEAPON_POSITIONS as u32 - 1;
            }
            slot = MAX_WEAPON_SLOTS as u32 - 1;
        }

        self.active = Select::None;
    }

    fn draw_bar(&self, x: c_int, y: c_int, color: RGB, f: f32) -> c_int {
        let mut x = x;
        let mut width = self.ab_width;
        let height = self.ab_height;
        let engine = self.engine;
        let f = f.clamp(0.0, 1.0);
        if f != 0.0 {
            let w = cmp::max(1, (f * width as f32) as c_int);
            engine.fill_rgba(x, y, w, height, RGB::GREENISH.rgba(255));
            x += w;
            width -= w;
        }
        engine.fill_rgba(x, y, width, height, color.rgba(128));
        x + width
    }

    fn draw_ammo_bar(&self, hud: &Hud, weapon: &Weapon, x: c_int, y: c_int) {
        if let Some(ammo) = weapon.ammo[0] {
            let inv = hud.inventory();
            let count = inv.ammo_count(ammo.ty);
            if count == 0 {
                return;
            }
            let f = count as f32 / ammo.max as f32;
            let x = self.draw_bar(x, y, hud.color(), f);

            if let Some(ammo) = weapon.ammo[1] {
                let count = inv.ammo_count(ammo.ty);
                let f = count as f32 / ammo.max as f32;
                self.draw_bar(x + 5, y, hud.color(), f);
            }
        }
    }
}

impl HudItem for WeaponMenu {
    fn vid_init(&mut self, hud: &Hud) {
        self.bucket0 = hud.find_sprite_index(c"bucket1");
        self.selection = hud.find_sprite(c"selection");

        let engine = self.engine;
        let screen = engine.screen_info();
        let scale = screen.scale();

        self.ab_width = 10 * scale as c_int;
        self.ab_height = 2 * scale as c_int;
    }

    fn reset(&mut self) {
        self.active = Select::None;
        self.last = Select::None;

        self.engine.unset_crosshair();
    }

    fn think(&mut self, hud: &Hud) {
        if self.active != Select::None && hud.key_bits() & consts::IN_ATTACK != 0 {
            let engine = self.engine;

            if let Select::Weapon(id, _, _) = self.active {
                let inv = hud.inventory();
                if let Some(weapon) = inv.weapon(id) {
                    engine.server_cmd(&weapon.name);
                    self.weapon_select = weapon.id;
                }
            }

            self.last = self.active;
            self.active = Select::None;

            hud.with_key_bits(|bits| bits & !consts::IN_ATTACK);

            engine.play_sound_by_name(c"common/wpn_select.wav", 1.0);
        }
    }

    fn draw(&mut self, hud: &Hud) {
        if !hud.has_suit() || hud.is_hidden(Hide::WEAPONS | Hide::ALL) {
            return;
        }

        let active_slot = match self.active {
            Select::Weapon(_, slot, _) => Some(slot),
            Select::Menu => None,
            Select::None => return,
        };

        let engine = self.engine;
        let inv = hud.inventory();
        let mut x = 10;
        let y = 10;
        for slot in 0..MAX_WEAPON_SLOTS as u32 {
            let a = match active_slot {
                Some(s) if s == slot => 255,
                _ => 192,
            };

            let color = hud.color().scale_color(a);

            if let Some(index) = self.bucket0 {
                let sprite = hud.sprites()[index + slot as usize];
                let width = match active_slot {
                    Some(s) if s == slot => inv
                        .get_first_pos(s)
                        .and_then(|weapon| weapon.active)
                        .map_or(sprite.width(), |s| s.width()),
                    _ => sprite.width(),
                };
                sprite.draw_additive(0, x, y, color);

                x += width + 5;
            }
        }

        let mut x = 10;

        let (bucket_width, bucket_height) = match self.bucket0 {
            Some(index) => hud.sprites()[index].size(),
            None => (0, 0),
        };

        for slot in 0..MAX_WEAPON_SLOTS as u32 {
            let mut y = bucket_height + 10;

            if matches!(active_slot, Some(s) if s == slot) {
                let width = match inv.get_first_pos(slot).and_then(|i| i.active) {
                    Some(sprite) => sprite.width(),
                    None => bucket_width,
                };

                for pos in 0..MAX_WEAPON_POSITIONS as u32 {
                    let Some(weapon) = inv.slot_weapon(slot, pos) else {
                        continue;
                    };

                    let color = hud.color();

                    if matches!(self.active, Select::Weapon(id, ..) if id == weapon.id) {
                        if let Some(sprite) = weapon.active {
                            sprite.draw_additive(0, x, y, color);
                        }

                        if let Some(sprite) = self.selection {
                            sprite.draw_additive(0, x, y, color);
                        }
                    } else if let Some(sprite) = weapon.inactive {
                        let color = if inv.has_ammo(weapon) {
                            color.scale_color(128)
                        } else {
                            RGB::REDISH.scale_color(96)
                        };

                        sprite.draw_additive(0, x, y, color);
                    }

                    self.draw_ammo_bar(hud, weapon, x + self.ab_width / 2, y);

                    y += weapon.active.unwrap().height() + 5;
                }

                x += width + 5;
            } else {
                for pos in 0..MAX_WEAPON_POSITIONS as u32 {
                    let Some(weapon) = inv.slot_weapon(slot, pos) else {
                        continue;
                    };

                    let color = if inv.has_ammo(weapon) {
                        hud.color().rgba(128)
                    } else {
                        RGB::REDISH.rgba(96)
                    };
                    engine.fill_rgba(x, y, bucket_width, bucket_height, color);
                    y += bucket_height + 5;
                }

                x += bucket_width + 5;
            }
        }
    }
}
