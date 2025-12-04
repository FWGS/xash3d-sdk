use core::{cmp, ffi::c_int, fmt::Write};

use alloc::vec::Vec;
use xash3d_client::{color::RGB, consts::MAX_PLAYERS, csz::CStrArray, prelude::*};

use crate::hud::{Hud, HudFlags, HudItem, PlayerInfoExtra};

#[derive(Default)]
struct Score {
    cl: u8,
    frags: i16,
    deaths: i16,
    #[allow(dead_code)]
    teamnumber: i16,
}

pub struct ScoreBoard {
    engine: ClientEngineRef,
    flags: HudFlags,
    scores: Vec<Score>,
}

impl ScoreBoard {
    pub fn new(engine: ClientEngineRef) -> Self {
        Self {
            engine,
            flags: HudFlags::INTERMISSION,
            scores: Vec::with_capacity(MAX_PLAYERS / 2),
        }
    }

    pub fn show(&mut self, value: bool) {
        self.flags.set(HudFlags::ACTIVE, value);
    }

    pub fn score_info(&mut self, cl: u8, extra: &PlayerInfoExtra) {
        let score = Score {
            cl,
            frags: extra.frags,
            deaths: extra.deaths,
            teamnumber: extra.teamnumber,
        };

        match self.scores.iter_mut().find(|i| i.cl == cl) {
            Some(i) => *i = score,
            None => self.scores.push(score),
        }

        self.scores.sort_by(|a, b| a.frags.cmp(&b.frags).reverse());
    }
}

impl HudItem for ScoreBoard {
    fn flags(&self) -> HudFlags {
        self.flags
    }

    fn init_hud_data(&mut self, _: &Hud) {
        self.scores.clear();
    }

    fn reset(&mut self) {
        self.flags.remove(HudFlags::ACTIVE);
    }

    fn draw(&mut self, hud: &Hud) {
        let engine = self.engine;
        let screen = engine.screen_info();
        let width = 70 * screen.char_width(b'w') as c_int;
        let width = cmp::min(screen.width().saturating_sub(40), width);
        let left = (screen.width() - width) / 2;
        let right = left + width;
        let top = screen.height() / 10;
        let bottom = screen.height() - top;

        let bg = RGB::BLACK.rgba(128);
        engine.fill_rgba_blend(left, top, right - left, bottom - top, bg);

        let gap = 30;
        let name_x = left + gap;
        let mut y = top + gap;
        engine.draw_string(name_x, y, &*hud.server_name(), hud.color());
        y += screen.char_height() * 2;

        let fields = [c"SCORE", c"DEATHS", c"PING", c"VOICE"];
        let mut w = 0;
        let mut h = 0;
        for &i in &fields {
            let (field_w, field_h) = engine.console_string_size(i);
            w = cmp::max(w, field_w);
            h = cmp::max(h, field_h);
        }
        let w = w + 8;
        let h = h + h / 4;

        let fields_x = right - gap - w * fields.len() as c_int;
        let mut x = fields_x;
        for &i in &fields {
            x += w;
            engine.set_text_color(hud.color());
            let (tw, th) = engine.console_string_size(i);
            engine.draw_console_string(x - tw, y + (h - th) / 2, i);
        }
        y += h;

        let local = unsafe { (*engine.get_local_player()).index } as usize;

        for score in self.scores.iter() {
            let cl = score.cl as usize;
            let Some(info) = engine.get_player_info(cl as c_int) else {
                continue;
            };

            if cl == local {
                let x = left + gap / 2;
                let w = right - left - gap;
                engine.fill_rgba_blend(x, y, w, h, hud.color().rgba(128));
            }

            let th = engine.console_string_height(info.name());
            engine.draw_console_string(name_x, y + (h - th) / 2, info.name());

            let mut x = fields_x;
            for i in [score.frags, score.deaths, info.ping() as i16, -1] {
                x += w;
                let mut buf = CStrArray::<256>::new();
                write!(buf.cursor(), "{i}").ok();
                let (tw, th) = engine.console_string_size(buf.as_c_str());
                engine.draw_console_string(x - tw, y + (h - th) / 2, buf.as_c_str());
            }
            y += h;
        }
    }
}
