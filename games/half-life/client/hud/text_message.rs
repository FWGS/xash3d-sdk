use core::ffi::{CStr, c_int};

use alloc::string::String;
use xash3d_client::{
    prelude::*,
    user_message::{UserMessageBuffer, UserMessageError, hook_user_message},
};

use crate::{export::hud, hud::say_text::SayText};

const HUD_PRINTNOTIFY: c_int = 1;
const HUD_PRINTCONSOLE: c_int = 2;
const HUD_PRINTTALK: c_int = 3;
const HUD_PRINTCENTER: c_int = 4;

fn cstr_copy(dst: &mut [u8], src: &[u8]) -> usize {
    let len = src.len() - src.ends_with(b"\0") as usize;
    let len = core::cmp::min(len, dst.len() - 1);
    dst[..len].copy_from_slice(&src[..len]);
    dst[len] = b'\0';
    len
}

pub struct TextMessage {}

impl TextMessage {
    pub fn new(engine: ClientEngineRef) -> Self {
        hook_user_message!(engine, TextMsg, TextMessage::msg_text);

        Self {}
    }

    fn msg_text(
        engine: ClientEngineRef,
        msg: &mut UserMessageBuffer,
    ) -> Result<(), UserMessageError> {
        const MSG_BUF_SIZE: usize = 128;

        let dest = msg.read_u8()? as c_int;
        let (dest, format) = lookup_string(engine, dest, msg.read_c_str()?);

        let mut strings = [[0; MSG_BUF_SIZE]; 4];
        for i in &mut strings {
            let Ok(s) = msg.read_c_str() else { break };
            let (_, s) = lookup_string(engine, 0, s);
            let len = cstr_copy(i, s.to_bytes());
            for c in i[..len].iter_mut().rev() {
                if *c == b'\r' || *c == b'\n' {
                    *c = b'\0';
                } else {
                    break;
                }
            }
        }

        let mut buffer = [0; MSG_BUF_SIZE];
        unsafe {
            let mut ptr = buffer.as_mut_ptr();
            let mut len = buffer.len();

            if dest == HUD_PRINTNOTIFY {
                ptr.cast::<u8>().write(1);
                ptr = ptr.add(1);
                len -= 1;
            }

            libc::snprintf(
                ptr.cast(),
                len,
                format.as_ptr(),
                strings[0].as_ptr(),
                strings[1].as_ptr(),
                strings[2].as_ptr(),
                strings[3].as_ptr(),
            );
        }
        buffer[buffer.len() - 1] = 0;
        convert_cr_to_nl(&mut buffer);
        let msg = CStr::from_bytes_until_nul(&buffer).unwrap();

        match dest {
            HUD_PRINTCENTER => engine.console_print(msg),
            HUD_PRINTNOTIFY => engine.console_print(msg),
            HUD_PRINTTALK => {
                let hud = hud();
                hud.items.get_mut::<SayText>().say_text(&hud, msg, -1);
            }
            HUD_PRINTCONSOLE => engine.console_print(msg),
            _ => {
                warn!("unimplemented text message dest={dest}");
            }
        }

        Ok(())
    }
}

pub fn localise_string(engine: ClientEngineRef, dst: &mut String, src: &str) {
    let mut cur = src;
    while !cur.is_empty() {
        while let Some(b'#') = cur.as_bytes().first() {
            cur = &cur[1..];

            let len = match cur.char_indices().find(|(_, c)| !c.is_ascii_alphanumeric()) {
                // "# "
                Some((0, _)) => {
                    dst.push('#');
                    continue;
                }
                // "#abc 123"
                Some((n, _)) => n,
                // "#abc"
                None => cur.len(),
            };

            let (head, tail) = cur.split_at(len);
            cur = tail;

            match engine.text_message_get(head) {
                Some(msg) => match unsafe { CStr::from_ptr(msg.pMessage) }.to_str() {
                    Ok(s) => dst.push_str(s),
                    Err(_) => {
                        warn!("invalid text message");
                        dst.push('#');
                        dst.push_str(head);
                    }
                },
                None => {
                    dst.push('#');
                    dst.push_str(head);
                }
            }
        }

        let offset = cur.find('#').unwrap_or(cur.len());
        let (head, tail) = cur.split_at(offset);
        dst.push_str(head);
        cur = tail;
    }
}

pub fn lookup_string(engine: ClientEngineRef, dest: c_int, msg: &CStr) -> (c_int, &CStr) {
    if !msg.to_bytes().starts_with(b"#") {
        return (dest, msg);
    }

    let s = unsafe { CStr::from_ptr(msg.as_ptr().offset(1)) };
    let Some(clmsg) = engine.text_message_get(s) else {
        return (dest, msg);
    };

    let mut dest = dest;
    if clmsg.effect < 0 {
        dest = -clmsg.effect;
    }

    (dest, unsafe { CStr::from_ptr(clmsg.pMessage) })
}

fn convert_cr_to_nl(s: &mut [u8]) {
    for c in s {
        if *c == b'\r' {
            *c = b'\n';
        }
    }
}
