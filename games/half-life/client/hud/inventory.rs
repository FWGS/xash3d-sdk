use core::{
    cmp,
    ffi::{CStr, c_int},
};

use xash3d_client::{
    color::RGB,
    csz::CStrArray,
    prelude::*,
    user_message::{UserMessageBuffer, UserMessageError, hook_user_message},
};
use xash3d_hl_shared::user_message;

use crate::{
    export::hud,
    hud::{MAX_WEAPONS, Sprite, Weapons, weapon_menu::WeaponMenu},
};

pub const MAX_WEAPON_POSITIONS: usize = MAX_WEAPON_SLOTS;
// hud item selection slots
pub const MAX_WEAPON_SLOTS: usize = 5;
pub const MAX_AMMO_TYPES: usize = 32;
pub const MAX_WEAPON_NAME: usize = 128;

const WEAPON_FLAGS_SELECTONEMPTY: u32 = 1;

#[derive(Copy, Clone)]
pub struct Ammo {
    pub ty: u32,
    pub max: u32,
    pub icon: Option<Sprite>,
}

impl Ammo {
    fn new(ty: i8, max: u8) -> Option<Self> {
        if ty >= 0 {
            Some(Self {
                ty: ty as u32,
                max: if max == 255 { u32::MAX } else { max as u32 },
                icon: None,
            })
        } else {
            None
        }
    }
}

#[derive(Copy, Clone)]
pub struct Weapon {
    engine: ClientEngineRef,

    pub name: CStrArray<MAX_WEAPON_NAME>,
    pub ammo: [Option<Ammo>; 2],
    pub slot: u32,
    pub slot_pos: u32,
    pub id: u32,
    pub flags: u32,
    pub clip: i32,

    pub active: Option<Sprite>,
    pub inactive: Option<Sprite>,
    pub crosshair: Option<Sprite>,
    pub autoaim: Option<Sprite>,
    pub zoomed_crosshair: Option<Sprite>,
    pub zoomed_autoaim: Option<Sprite>,
}

impl Weapon {
    pub fn read_from_message(
        engine: ClientEngineRef,
        msg: &mut UserMessageBuffer,
    ) -> Result<Self, UserMessageError> {
        let msg = msg.read::<user_message::WeaponList>()?;
        let ammo1 = msg.ammo1;
        let max1 = msg.max1;
        let ammo2 = msg.ammo2;
        let max2 = msg.max2;
        let slot = msg.slot as i32 as u32;
        let slot_pos = msg.slot_pos as i32 as u32;
        let id = msg.id as i32 as u32;
        let flags = msg.flags as u32;
        let clip = 0;

        if id as usize >= MAX_WEAPONS {
            return Err(UserMessageError::InvalidNumber);
        }

        if slot as usize > MAX_WEAPON_SLOTS {
            return Err(UserMessageError::InvalidNumber);
        }

        if slot_pos as usize > MAX_WEAPON_POSITIONS {
            return Err(UserMessageError::InvalidNumber);
        }

        if ammo1 < -1 || ammo1 >= MAX_AMMO_TYPES as i8 {
            return Err(UserMessageError::InvalidNumber);
        }

        if ammo2 < -1 || ammo2 >= MAX_AMMO_TYPES as i8 {
            return Err(UserMessageError::InvalidNumber);
        }

        if (ammo1 >= 0 && max1 == 0) || (ammo2 >= 0 && max2 == 0) {
            return Err(UserMessageError::InvalidNumber);
        }

        let ammo1 = Ammo::new(ammo1, max1);
        let ammo2 = Ammo::new(ammo2, max2);

        Ok(Self {
            engine,

            name: CStrArray::try_from(msg.name).unwrap(),
            ammo: [ammo1, ammo2],
            slot,
            slot_pos,
            id,
            flags,
            clip,

            active: None,
            inactive: None,
            crosshair: None,
            autoaim: None,
            zoomed_crosshair: None,
            zoomed_autoaim: None,
        })
    }

    fn load_sptires(&mut self) {
        let engine = self.engine;
        let list = engine.spr_get_list(format_args!("sprites/{}.txt", self.name));
        if list.is_empty() {
            return;
        }

        let screen = engine.screen_info();
        let res = screen.sprite_resolution() as c_int;

        let load = |name: &CStr| {
            list.find(name.into(), res)
                .and_then(|i| {
                    engine
                        .spr_load(format_args!("sprites/{}.spr", i.sprite()))
                        .map(|s| (s, i.rc))
                })
                .map(|(s, rc)| Sprite::new(s, rc))
        };

        self.crosshair = load(c"crosshair");
        self.autoaim = load(c"autoaim");
        self.zoomed_crosshair = load(c"zoom").or(self.crosshair);
        self.zoomed_autoaim = load(c"zoom_autoaim").or(self.zoomed_crosshair);
        self.inactive = load(c"weapon");
        self.active = load(c"weapon_s");

        for (i, ammo) in self.ammo.iter_mut().enumerate() {
            if let Some(ammo) = ammo {
                let name = if i == 0 { c"ammo" } else { c"ammo2" };
                ammo.icon = load(name);
            }
        }
    }

    fn max_sprite_height(&self) -> c_int {
        let mut gap = 0;
        if let Some(s) = self.inactive {
            gap = cmp::max(gap, s.height());
        }
        for i in self.ammo.iter().filter_map(|i| i.as_ref()) {
            if let Some(s) = i.icon {
                gap = cmp::max(gap, s.height());
            }
        }
        gap
    }
}

pub struct Inventory {
    engine: ClientEngineRef,

    list: [Option<Weapon>; MAX_WEAPONS],
    slots: [[Option<u32>; MAX_WEAPON_POSITIONS]; MAX_WEAPON_SLOTS],
    ammo: [u32; MAX_AMMO_TYPES],
    current: Option<u32>,
    max_height: c_int,

    /// The weapons that player is carrying.
    weapons_old: Weapons,
    weapons: Weapons,
}

impl Inventory {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_user_message!(engine, WeaponList, |engine, msg| {
            match Weapon::read_from_message(engine, msg) {
                Ok(weapon) => {
                    hud().inventory_mut().weapon_add(weapon);
                    true
                }
                Err(_) => {
                    error!("WeaponList: invalid data");
                    false
                }
            }
        });

        hook_user_message!(engine, AmmoX, |_, msg| {
            let msg = msg.read::<user_message::AmmoX>()?;
            hud()
                .inventory_mut()
                .ammo_set(msg.ty.into(), msg.count.into());
            Ok(())
        });

        hook_user_message!(engine, CurWeapon, |engine, msg| {
            let msg = msg.read::<user_message::CurWeapon>()?;
            Ok(Inventory::msg_cur_weapon(
                engine, msg.state, msg.id, msg.clip,
            ))
        });

        hook_user_message!(engine, HideWeapon, |engine, msg| {
            use super::Hide;

            let msg = msg.read::<user_message::HideWeapon>()?;
            let hud = hud();
            hud.set_hide(Hide::from_bits_retain(msg.hide as u32));

            if !engine.is_spectator_only() {
                if hud.is_hidden(Hide::WEAPONS | Hide::ALL) {
                    hud.items.get_mut::<WeaponMenu>().close();
                    engine.unset_crosshair();
                } else {
                    hud.inventory_mut().set_crosshair();
                }
            }

            Ok(())
        });

        Self {
            engine,

            list: Default::default(),
            slots: Default::default(),
            ammo: Default::default(),
            current: None,
            max_height: 0,

            weapons_old: Weapons::empty(),
            weapons: Weapons::empty(),
        }
    }

    pub fn vid_init(&mut self) {
        for i in self.list.iter_mut().filter_map(|i| i.as_mut()) {
            i.load_sptires();
        }

        self.max_height = 0;
        for weapon in self.list.iter().filter_map(|i| i.as_ref()) {
            self.max_height = cmp::max(self.max_height, weapon.max_sprite_height());
        }
    }

    pub fn reset(&mut self) {
        self.slots.fill(Default::default());
        self.ammo.fill(0);
        self.current = None;
        self.weapons_old = Weapons::empty();
        self.weapons = Weapons::empty();
    }

    pub fn think(&mut self) {
        if self.weapons_old != self.weapons {
            self.weapons_old = self.weapons;

            for i in 0..MAX_WEAPONS as u32 {
                if let Some(weapon) = self.weapon(i) {
                    if self.weapons.bits() & (1 << weapon.id) != 0 {
                        self.weapon_pickup(weapon.id);
                    } else {
                        self.weapon_drop(weapon.id);
                    }
                }
            }
        }
    }

    pub fn pickup_gap(&self) -> c_int {
        self.max_height + 5
    }

    pub fn pickup_height(&self) -> c_int {
        32 + self.max_height * 2
    }

    pub fn weapons(&self) -> Weapons {
        self.weapons
    }

    pub fn weapons_set(&mut self, weapons: Weapons) {
        self.weapons = weapons;
    }

    pub fn weapon_add(&mut self, mut weapon: Weapon) {
        weapon.load_sptires();
        self.max_height = cmp::max(self.max_height, weapon.max_sprite_height());
        self.list[weapon.id as usize] = Some(weapon);
    }

    pub fn weapon(&self, id: u32) -> Option<&Weapon> {
        self.list[id as usize].as_ref()
    }

    pub fn current(&self) -> Option<&Weapon> {
        self.weapon(self.current?)
    }

    pub fn slot_weapon_id(&self, slot: u32, pos: u32) -> Option<u32> {
        self.slots.get(slot as usize)?.get(pos as usize).copied()?
    }

    pub fn slot_weapon(&self, slot: u32, pos: u32) -> Option<&Weapon> {
        let id = self.slot_weapon_id(slot, pos)?;
        self.weapon(id)
    }

    pub fn ammo_set(&mut self, ty: u32, count: u32) {
        self.ammo[ty as usize] = count;
    }

    pub fn ammo_count(&self, ty: u32) -> u32 {
        self.ammo[ty as usize]
    }

    pub fn ammo_icon(&self, ty: u32) -> Option<Sprite> {
        for weapon in self.list.iter().filter_map(|i| i.as_ref()) {
            for ammo in weapon.ammo.iter().filter_map(|i| i.as_ref()) {
                if ammo.ty == ty {
                    return ammo.icon;
                }
            }
        }
        None
    }

    pub fn weapon_icon(&self, ty: u32) -> Option<Sprite> {
        self.list
            .get(ty as usize)?
            .and_then(|weapon| weapon.inactive)
    }

    fn weapon_pickup(&mut self, id: u32) {
        if let Some(weapon) = self.weapon(id) {
            self.slots[weapon.slot as usize][weapon.slot_pos as usize] = Some(id);
        }
    }

    fn weapon_drop(&mut self, id: u32) {
        if let Some(weapon) = self.weapon(id) {
            self.slots[weapon.slot as usize][weapon.slot_pos as usize] = None;
        }
    }

    pub fn has_ammo(&self, weapon: &Weapon) -> bool {
        if weapon.flags & WEAPON_FLAGS_SELECTONEMPTY != 0 {
            return true;
        }

        if weapon.ammo[0].is_none() || weapon.clip > 0 {
            return true;
        }

        if let Some(u32::MAX) = weapon.ammo[0].map(|i| i.max) {
            return true;
        }

        weapon
            .ammo
            .iter()
            .filter_map(|i| i.as_ref())
            .any(|i| self.ammo_count(i.ty) != 0)
    }

    pub fn get_first_pos(&self, slot: u32) -> Option<&Weapon> {
        for i in 0..MAX_WEAPON_POSITIONS as u32 {
            let Some(id) = self.slot_weapon_id(slot, i) else {
                continue;
            };
            let Some(weapon) = self.weapon(id) else {
                continue;
            };
            if self.has_ammo(weapon) {
                return Some(weapon);
            }
        }
        None
    }

    pub fn get_next_active_pos(&self, slot: u32, slot_pos: u32) -> Option<&Weapon> {
        if slot as usize >= MAX_WEAPON_SLOTS || slot_pos as usize >= MAX_WEAPON_POSITIONS {
            return None;
        }

        match self.slot_weapon(slot, slot_pos + 1) {
            Some(weapon) if self.has_ammo(weapon) => Some(weapon),
            _ => self.get_next_active_pos(slot, slot_pos + 1),
        }
    }

    pub fn set_crosshair(&self) {
        if let Some(weapon) = self.current() {
            if let Some(s) = weapon.crosshair {
                self.engine.set_crosshair(s.handle(), s.rect(), RGB::WHITE);
            }
        }
    }

    fn msg_cur_weapon(engine: ClientEngineRef, state: u8, id: i8, clip: i8) -> bool {
        if id < 1 {
            engine.unset_crosshair();
            return false;
        }

        // TODO: if g_iUser1 != OBS_IN_EYE

        let hud = hud();
        let mut weapons = hud.inventory_mut();
        let Some(weapon) = &mut weapons.list[id as usize] else {
            return false;
        };

        if clip < -1 {
            weapon.clip = clip.unsigned_abs() as i32;
        } else {
            weapon.clip = clip as i32;
        }

        if state == 0 {
            // it's not the current weapon
            return true;
        }

        hud.items.get_mut::<super::ammo::Ammo>().fade_start();

        let (autoaim, crosshair) = if hud.fov() >= 90 {
            (weapon.autoaim, weapon.crosshair)
        } else {
            (weapon.zoomed_autoaim, weapon.zoomed_crosshair)
        };
        weapons.current = Some(id as u32);

        let on_target = state > 1;
        if let (true, Some(s)) = (on_target, autoaim) {
            engine.set_crosshair(s.handle(), s.rect(), RGB::WHITE);
        } else if let Some(s) = crosshair {
            engine.set_crosshair(s.handle(), s.rect(), RGB::WHITE);
        } else {
            engine.unset_crosshair();
        }

        true
    }
}
