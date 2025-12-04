use core::{cmp, ffi::c_int};

use xash3d_client::{prelude::*, user_message::hook_user_message};
use xash3d_hl_shared::user_message;

use crate::{
    export::hud,
    hud::{Fade, Hide, Hud, Sprite},
};

pub struct Battery {
    engine: ClientEngineRef,
    fade: Fade,
    current: i16,
    suit_empty: Option<Sprite>,
    suit_full: Option<Sprite>,
}

impl Battery {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_user_message!(engine, Battery, |_, msg| {
            let msg = msg.read::<user_message::Battery>()?;
            hud().items.get_mut::<Battery>().set(msg.battery);
            Ok(())
        });

        Self {
            engine,
            current: 0,
            fade: Fade::default(),
            suit_empty: None,
            suit_full: None,
        }
    }

    pub fn set(&mut self, value: i16) {
        // TODO: set active???

        if self.current != value {
            self.current = value;
            self.fade.start();
        }
    }
}

impl super::HudItem for Battery {
    fn vid_init(&mut self, hud: &Hud) {
        self.suit_empty = hud.find_sprite(c"suit_empty");
        self.suit_full = hud.find_sprite(c"suit_full");
    }

    fn draw(&mut self, hud: &Hud) {
        let engine = self.engine;
        if hud.is_hidden(Hide::HEALTH) || engine.is_spectator_only() || !hud.has_suit() {
            return;
        }

        let (Some(empty), Some(full)) = (self.suit_empty, self.suit_full) else {
            warn!("suit sprites was not loaded");
            return;
        };

        let digits = hud.digits();
        let color = hud.color().scale_color(self.fade.alpha(hud.time_delta()));
        let screen_info = engine.screen_info();
        let width = empty.width();
        let mut x = width * 3;
        let mut y = screen_info.height() - digits.height() - digits.height() / 2;
        let offset = empty.height() / 6;

        empty.draw_additive(0, x, y - offset, color);

        let height = (full.rect().bottom - empty.rect().top) as f32;
        let mut rc = full.rect();
        rc.top += (height * ((100 - cmp::min(100, self.current)) as f32 * 0.01)) as c_int;
        if rc.bottom > rc.top {
            let y = y + (rc.top - full.rect().top);
            full.handle()
                .draw_additive_rect(0, x, y - offset, color, rc);
        }

        x += width;
        y += (digits.height() as f32 * 0.2) as c_int;
        hud.draw_number(self.current as c_int)
            .width(3)
            .color(color)
            .at(x, y);
    }
}
