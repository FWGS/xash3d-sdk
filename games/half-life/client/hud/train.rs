use core::ffi::c_int;

use xash3d_client::{prelude::*, sprite::SpriteHandle, user_message::hook_user_message};
use xash3d_hl_shared::user_message;

use crate::{
    export::hud,
    hud::{Hud, HudItem, try_spr_load},
};

pub struct Train {
    engine: ClientEngineRef,
    pos: u8,
    sprite: Option<SpriteHandle>,
}

impl Train {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_user_message!(engine, Train, |_, msg| {
            let msg = msg.read::<user_message::Train>()?;
            hud().items.get_mut::<Train>().set_pos(msg.pos);
            Ok(())
        });

        Self {
            engine,
            pos: 0,
            sprite: None,
        }
    }

    fn set_pos(&mut self, pos: u8) {
        self.pos = pos;
    }
}

impl HudItem for Train {
    fn draw(&mut self, hud: &Hud) {
        if self.pos == 0 {
            return;
        }

        let engine = self.engine;
        if self.sprite.is_none() {
            self.sprite = try_spr_load(hud.sprite_resolution(), |res| {
                engine.spr_load(format_args!("sprites/{res}_train.spr"))
            });
        }

        let Some(sprite) = self.sprite else { return };
        let (w, h) = sprite.size(0);
        let screen = engine.screen_info();
        let x = screen.width() / 3 + w / 4;
        let y = screen.height() - h - hud.digits().height();
        sprite.draw_additive(self.pos as c_int - 1, x, y, hud.color());
    }
}
