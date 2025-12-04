use core::ffi::c_int;

use xash3d_client::prelude::*;

use crate::hud::{Fade, Hide, Hud, HudItem};

const FALLBACK_WIDTH: c_int = 24;

pub struct Ammo {
    engine: ClientEngineRef,
    fade: Fade,
}

impl Ammo {
    pub fn new(engine: ClientEngineRef) -> Self {
        Self {
            engine,
            fade: Fade::new(super::FADE_TIME_AMMO),
        }
    }

    pub fn fade_start(&mut self) {
        self.fade.start()
    }
}

impl HudItem for Ammo {
    fn reset(&mut self) {
        self.fade.stop();
    }

    fn draw(&mut self, hud: &Hud) {
        if !hud.has_suit() || hud.is_hidden(Hide::WEAPONS) {
            return;
        }

        let inv = hud.inventory();
        let Some(weapon) = inv.current() else {
            return;
        };

        if !weapon.ammo.iter().any(|i| i.is_some()) {
            return;
        }

        let a = self.fade.alpha(hud.time_delta());
        let color = hud.color().scale_color(a);

        let engine = self.engine;
        let screen = engine.screen_info();
        let digits = hud.digits();
        let ammo_width = digits.width();

        let mut y = screen.height() - digits.height() - digits.height() / 2;
        y += (digits.height() as f32 * 0.2) as c_int;

        if let Some(ammo) = weapon.ammo[0] {
            let ammo_count = inv.ammo_count(ammo.ty) as c_int;
            let icon_width = ammo.icon.map_or(FALLBACK_WIDTH, |s| s.width());

            let mut x = screen.width() - icon_width;
            if weapon.clip >= 0 {
                x -= 8 * ammo_width;
                x = hud.draw_number(weapon.clip).width(3).color(color).at(x, y);

                let bar_width = ammo_width / 10;
                x += ammo_width / 2;
                engine.fill_rgba(x, y, bar_width, digits.height(), hud.color().rgba(a));

                x += ammo_width / 2 + bar_width;
                x = hud.draw_number(ammo_count).width(3).color(color).at(x, y);
            } else {
                x -= 4 * ammo_width;
                x = hud.draw_number(ammo_count).width(3).color(color).at(x, y);
            }

            if let Some(icon) = ammo.icon {
                let offset = icon.height() / 8;
                icon.draw_additive(0, x, y - offset, color);
            }
        }

        if let Some(ammo) = weapon.ammo[1] {
            let ammo_count = inv.ammo_count(ammo.ty) as c_int;
            if ammo_count > 0 {
                let icon_width = ammo.icon.map_or(FALLBACK_WIDTH, |s| s.width());

                let mut x = screen.width() - icon_width;
                y -= digits.height() + digits.height() / 4;

                x -= 4 * ammo_width;
                x = hud.draw_number(ammo_count).width(3).color(color).at(x, y);

                if let Some(icon) = ammo.icon {
                    let offset = icon.height() / 8;
                    icon.draw_additive(0, x, y - offset, color);
                }
            }
        }
    }
}
