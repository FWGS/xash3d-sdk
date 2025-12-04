use core::fmt::Write;

use xash3d_client::{csz::CStrArray, prelude::*, user_message::hook_user_message};
use xash3d_hl_shared::user_message;

use crate::{
    export::hud,
    hud::{Hud, HudItem},
};

// cap geiger sounds by 60 fps
const LIFE: f32 = 1.0 / 60.0;

pub struct Geiger {
    engine: ClientEngineRef,
    range: u16,
    time: f32,
}

impl Geiger {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_user_message!(engine, Geiger, |_, msg| {
            let msg = msg.read::<user_message::Geiger>()?;
            hud().items.get_mut::<Geiger>().range = (msg.range as u16) << 2;
            Ok(())
        });

        Self {
            engine,
            range: 0,
            time: 0.0,
        }
    }
}

impl HudItem for Geiger {
    fn reset(&mut self) {
        self.time = 0.0;
    }

    fn draw(&mut self, hud: &Hud) {
        if !(1..1000).contains(&self.range) {
            return;
        }

        let now = hud.time();
        if hud.time_delta() != 0.0 && self.time >= now {
            return;
        }
        self.time = now + LIFE;

        let pct;
        let vol;
        let i;

        match self.range {
            801.. => {
                pct = 0;
                vol = 0.0;
                i = 0;
            }
            601.. => {
                pct = 2;
                vol = 0.4;
                i = 2;
            }
            501.. => {
                pct = 4;
                vol = 0.5;
                i = 2;
            }
            401.. => {
                pct = 8;
                vol = 0.6;
                i = 3;
            }
            301.. => {
                pct = 8;
                vol = 0.7;
                i = 3;
            }
            201.. => {
                pct = 28;
                vol = 0.78;
                i = 3;
            }
            151.. => {
                pct = 40;
                vol = 0.8;
                i = 3;
            }
            101.. => {
                pct = 60;
                vol = 0.85;
                i = 3;
            }
            76.. => {
                pct = 80;
                vol = 0.9;
                i = 3;
            }
            51.. => {
                pct = 90;
                vol = 0.95;
                i = 2;
            }
            _ => {
                pct = 95;
                vol = 1.0;
                i = 2;
            }
        }

        let engine = self.engine;
        let rand = || engine.rand();
        let vol = vol * (rand() & 127) as f32 / 255.0 + 0.25;

        if rand() & 127 < pct || rand() & 127 < pct {
            let mut j = rand() & 1;
            if i > 2 {
                j += rand() & 1;
            }
            let mut buf = CStrArray::<64>::new();
            write!(buf.cursor(), "player/geiger{}.wav", j + 1).ok();
            engine.play_sound_by_name(buf.as_c_str(), vol);
        }
    }
}
