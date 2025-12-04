use core::{
    cmp,
    ffi::{CStr, c_int},
    fmt::Write,
};

use alloc::collections::vec_deque::VecDeque;
use xash3d_client::{
    color::RGB,
    csz::{CStrArray, CStrThin},
    cvar::{self, Cvar},
    prelude::*,
    user_message::hook_user_message,
};
use xash3d_hl_shared::user_message;

use crate::{
    export::hud,
    hud::{Hud, HudItem, MAX_PLAYER_NAME_LENGTH, Sprite},
};

const MAX_DEATH_NOTICES: usize = 4;
const DEATHNOTICE_TOP: c_int = 32;

#[derive(Copy, Clone)]
struct Player {
    name: CStrArray<{ MAX_PLAYER_NAME_LENGTH * 2 }>,
    color: RGB,
}

impl Player {
    fn new(name: &CStrThin, color: RGB) -> Self {
        Self {
            name: name.try_into().unwrap(),
            color,
        }
    }

    fn name(&self) -> &CStr {
        self.name.as_c_str()
    }
}

#[derive(Copy, Clone)]
enum Victim {
    Player(Player),
    Object {
        name: CStrArray<{ MAX_PLAYER_NAME_LENGTH * 2 }>,
    },
}

impl Victim {
    fn name(&self) -> &CStr {
        match self {
            Self::Player(p) => p.name(),
            Self::Object { name } => name.as_c_str(),
        }
    }

    fn color(&self) -> Option<RGB> {
        match self {
            Self::Player(p) => Some(p.color),
            Self::Object { .. } => None,
        }
    }
}

#[derive(Copy, Clone)]
struct Notice {
    killer: Option<Player>,
    victim: Victim,
    weapon: Option<Sprite>,
    team_kill: bool,
    display_time: f32,
}

pub struct DeathNotice {
    engine: ClientEngineRef,
    list: VecDeque<Notice>,
    skull: Option<Sprite>,

    hud_deathnotice_time: Cvar,
}

impl DeathNotice {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_user_message!(engine, DeathMsg, |_, msg| {
            let msg = msg.read::<user_message::DeathMsg>()?;
            let hud = hud();
            hud.items.get_mut::<DeathNotice>().death(&hud, &msg);
            Ok(())
        });

        Self {
            engine,
            list: Default::default(),
            skull: None,

            hud_deathnotice_time: engine
                .create_cvar(c"hud_deathnotice_time", c"6", cvar::ARCHIVE)
                .unwrap(),
        }
    }

    fn death(&mut self, hud: &Hud, msg: &user_message::DeathMsg) {
        let killer_id = msg.killer;
        let victim_id = msg.victim;
        let killed_with: &CStrThin = msg.killed_with.into();
        // TODO: spectator.death_message(victim);

        let engine = self.engine;
        let suicide = killer_id == victim_id || killer_id == 0;
        let team_kill = killed_with == c"d_teammate";

        let killer = if !suicide {
            engine.get_player_info(killer_id as c_int).map(|info| {
                let color = hud.get_client_color(killer_id as c_int);
                Player::new(info.name(), color)
            })
        } else {
            None
        };

        let player_kill = victim_id != u8::MAX;
        let victim = if player_kill {
            let info = engine.get_player_info(victim_id as c_int);
            let name = info.as_ref().map_or(c"unknown".into(), |info| info.name());
            let color = hud.get_client_color(victim_id as c_int);
            Victim::Player(Player::new(name, color))
        } else {
            Victim::Object {
                name: killed_with.try_into().unwrap(),
            }
        };

        let weapon = {
            let mut buf = CStrArray::<128>::new();
            write!(buf.cursor(), "d_{killed_with}").ok();
            hud.find_sprite(buf)
        };

        let display_time = hud.time() + self.hud_deathnotice_time.get();

        match victim {
            Victim::Player(victim) => {
                if let Some(ref killer) = killer {
                    engine.console_print(killer.name());
                    if team_kill {
                        engine.console_print(c" killed his teammate ");
                    } else {
                        engine.console_print(c" killed ");
                    }
                    engine.console_print(victim.name());
                } else {
                    engine.console_print(victim.name());
                    if killed_with == c"world" {
                        engine.console_print(c" died");
                    } else {
                        engine.console_print(c" killed self");
                    }
                }

                if !killed_with.is_empty() && killed_with != c"world" && !team_kill {
                    engine.console_print(c" with ");

                    if killed_with == c"egon" {
                        engine.console_print(c"gluon gun");
                    } else if killed_with == c"gauss" {
                        engine.console_print(c"tau cannon");
                    } else {
                        engine.console_print(killed_with);
                    }
                }
            }
            Victim::Object { .. } => {
                let name = killer.as_ref().map_or(c"unknown", |i| i.name());
                engine.console_print(name);
                engine.console_print(c" killed a ");
                engine.console_print(victim.name());
            }
        }
        engine.console_print(c"\n");

        let notice = Notice {
            killer,
            victim,
            weapon,
            team_kill,
            display_time,
        };

        while self.list.len() >= MAX_DEATH_NOTICES {
            self.list.pop_front();
        }
        self.list.push_back(notice);
    }
}

impl HudItem for DeathNotice {
    fn vid_init(&mut self, hud: &Hud) {
        self.list.clear();
        self.skull = hud.find_sprite(c"d_skull");
    }

    fn init_hud_data(&mut self, _: &Hud) {
        self.list.clear();
    }

    fn draw(&mut self, hud: &Hud) {
        // TODO: exit if !viewport.allowed_to_print_text()

        while let Some(i) = self.list.front() {
            if i.display_time >= hud.time() {
                break;
            }
            self.list.pop_front();
        }

        let engine = self.engine;
        let screen = engine.screen_info();
        let gap = cmp::max(screen.char_height(), self.skull.map_or(20, |s| s.height()));

        let x = screen.width() - 4;
        let mut y = DEATHNOTICE_TOP + 6;

        for notice in &self.list {
            let mut x = x;

            // draw victim
            let name = notice.victim.name();
            x -= engine.console_string_width(name);
            engine.set_text_color(notice.victim.color().unwrap_or(RGB::WHITE));
            engine.draw_console_string(x, y, name);

            // draw weapon
            if let Some(s) = notice.weapon.or(self.skull) {
                let color = if notice.team_kill {
                    RGB::new(10, 240, 10) // TODO:
                } else {
                    RGB::new(255, 80, 0) // TODO:
                };

                x -= s.width();
                s.draw_additive(0, x, y - 4, color);
            }

            // draw killer name
            if let Some(killer) = notice.killer {
                x -= 5;
                x -= engine.console_string_width(killer.name());
                engine.set_text_color(killer.color);
                engine.draw_console_string(x, y, killer.name());
            }

            y += gap;
        }
    }
}
