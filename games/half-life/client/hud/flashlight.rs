use core::ffi::c_int;

use xash3d_client::{color::RGB, prelude::*, user_message::hook_user_message};
use xash3d_hl_shared::user_message;

use crate::{
    export::hud,
    hud::{Hide, Hud, Sprite},
};

pub struct Flashlight {
    engine: ClientEngineRef,
    battery_f: f32,
    battery: u8,
    enabled: bool,
    flash_empty: Option<Sprite>,
    flash_full: Option<Sprite>,
    flash_beam: Option<Sprite>,
}

impl Flashlight {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_user_message!(engine, FlashBat, |_, msg| {
            let msg = msg.read::<user_message::FlashBat>()?;
            hud().items.get_mut::<Flashlight>().set(msg.battery);
            Ok(())
        });

        hook_user_message!(engine, Flashlight, |_, msg| {
            let msg = msg.read::<user_message::Flashlight>()?;
            let hud = hud();
            let mut flash = hud.items.get_mut::<Flashlight>();
            flash.enabled(msg.on);
            flash.set(msg.battery);
            Ok(())
        });

        Self {
            engine,
            battery_f: 0.0,
            battery: 0,
            enabled: false,
            flash_empty: None,
            flash_full: None,
            flash_beam: None,
        }
    }

    pub fn set(&mut self, value: u8) {
        if self.battery != value {
            self.battery = value;
            self.battery_f = value as f32 / 100.0;
        }
    }

    pub fn enabled(&mut self, value: bool) {
        self.enabled = value;
    }
}

impl super::HudItem for Flashlight {
    fn vid_init(&mut self, hud: &Hud) {
        self.flash_empty = hud.find_sprite(c"flash_empty");
        self.flash_full = hud.find_sprite(c"flash_full");
        self.flash_beam = hud.find_sprite(c"flash_beam");
    }

    fn reset(&mut self) {
        self.enabled = false;
    }

    fn draw(&mut self, hud: &Hud) {
        if hud.is_hidden(Hide::FLASHLIGHT) || !hud.has_suit() {
            return;
        }

        let (Some(empty), Some(full), Some(beam)) =
            (self.flash_empty, self.flash_full, self.flash_beam)
        else {
            return;
        };

        let a = if self.enabled { 225 } else { super::MIN_ALPHA };

        let color = if self.battery < 20 {
            RGB::REDISH
        } else {
            hud.color()
        };
        let color = color.scale_color(a);

        let engine = self.engine;
        let screen = engine.screen_info();

        let width = empty.width();
        let x = screen.width() - width - width / 2;
        let y = (empty.rect().bottom - full.rect().top) / 2;

        empty.draw_additive(0, x, y, color);

        if self.enabled {
            let x = screen.width() - width / 2;
            beam.draw_additive(0, x, y, color);
        }

        let offset = (width as f32 * (1.0 - self.battery_f)) as c_int;
        if offset < width {
            let mut rect = full.rect();
            rect.left += offset;
            full.handle()
                .draw_additive_rect(0, x + offset, y, color, rect);
        }
    }
}
