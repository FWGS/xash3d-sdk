use core::ffi::c_int;

use xash3d_client::{
    cvar::{self, Cvar},
    prelude::*,
    user_message::hook_user_message,
};
use xash3d_hl_shared::user_message;

use crate::{
    export::hud,
    hud::{Hide, Hud, HudItem},
};

const MAX_HISTORY: usize = 12;

#[derive(Copy, Clone, Debug)]
enum ItemKind {
    Ammo(u8, u8),
    Weapon(u8),
    Item(Option<usize>),
}

#[derive(Copy, Clone, Debug)]
struct Item {
    time: f32,
    kind: ItemKind,
}

pub struct History {
    engine: ClientEngineRef,
    items: [Option<Item>; MAX_HISTORY],
    slot: usize,

    hud_drawhistory_time: Cvar,
}

impl History {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_user_message!(engine, AmmoPickup, |_, msg| {
            let msg = msg.read::<user_message::AmmoPickup>()?;
            if msg.count != 0 {
                let hud = hud();
                hud.items
                    .get_mut::<History>()
                    .add(&hud, ItemKind::Ammo(msg.index, msg.count));
            }
            Ok(())
        });

        hook_user_message!(engine, WeapPickup, |_, msg| {
            let msg = msg.read::<user_message::WeapPickup>()?;
            let hud = hud();
            hud.items
                .get_mut::<History>()
                .add(&hud, ItemKind::Weapon(msg.index));
            Ok(())
        });

        hook_user_message!(engine, ItemPickup, |_, msg| {
            let msg = msg.read::<user_message::ItemPickup>()?;
            let hud = hud();
            let index = hud.find_sprite_index(msg.classname);
            hud.items
                .get_mut::<History>()
                .add(&hud, ItemKind::Item(index));
            Ok(())
        });

        Self {
            engine,
            items: [None; MAX_HISTORY],
            slot: 0,

            hud_drawhistory_time: engine
                .create_cvar(c"hud_drawhistory_time", c"5", cvar::NO_FLAGS)
                .unwrap(),
        }
    }

    fn add(&mut self, hud: &Hud, kind: ItemKind) {
        let engine = self.engine;
        let inv = hud.inventory();
        let height = self.slot as c_int * inv.pickup_gap() + inv.pickup_height();
        let height_max = engine.screen_info().height() - 100;
        if height > height_max || self.slot >= self.items.len() {
            self.slot = 0;
        }
        self.items[self.slot] = Some(Item {
            time: hud.time() + self.hud_drawhistory_time.get(),
            kind,
        });
        self.slot += 1;
    }
}

impl HudItem for History {
    fn reset(&mut self) {
        self.items.fill(None);
    }

    fn draw(&mut self, hud: &Hud) {
        if !hud.has_suit() || hud.is_hidden(Hide::WEAPONS | Hide::ALL) {
            return;
        }

        let inv = hud.inventory();
        let gap = inv.pickup_gap();
        let height = inv.pickup_height();

        let engine = self.engine;
        let screen = engine.screen_info();
        let now = hud.time();

        for i in 0..self.items.len() {
            let item = match self.items[i] {
                Some(item) => {
                    if item.time <= now {
                        self.items[i] = None;
                        if self.items.iter().all(|i| i.is_none()) {
                            self.slot = 0;
                        }
                        continue;
                    }
                    item
                }
                None => continue,
            };

            let a = ((item.time - now) * 80.0) as u8;
            let color = hud.color().scale_color(a);
            let mut x = screen.width() - 4;
            let y = screen.height() - height - gap * i as c_int;

            match item.kind {
                ItemKind::Ammo(index, count) => {
                    let Some(icon) = inv.ammo_icon(index as u32) else {
                        continue;
                    };

                    x -= icon.width();
                    icon.draw_additive(0, x, y, color);

                    hud.draw_number(count as i32)
                        .color(color)
                        .string(true)
                        .reverse(true)
                        .at(x - 10, y);
                }
                ItemKind::Weapon(index) => {
                    let Some(icon) = inv.weapon_icon(index as u32) else {
                        continue;
                    };

                    x -= icon.width();
                    icon.draw_additive(0, x, y, color);
                }
                ItemKind::Item(index) => {
                    let Some(index) = index else { continue };
                    let Some(icon) = hud.sprite(index) else {
                        continue;
                    };

                    x -= icon.width();
                    icon.draw_additive(0, x, y, color);
                }
            }
        }
    }
}
