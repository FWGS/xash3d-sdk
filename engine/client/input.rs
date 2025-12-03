use core::cell::UnsafeCell;

use xash3d_shared::ffi::common::kbutton_t;

use crate::prelude::*;

pub use xash3d_shared::input::*;

/// Key button data.
pub struct KeyButtonData {
    raw: UnsafeCell<kbutton_t>,
    bits: u32,
}

// SAFETY: only used by KeyButton from the engine thread
unsafe impl Sync for KeyButtonData {}

impl KeyButtonData {
    /// Creates a new key button data.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self::with_bits(0)
    }

    /// Creates a new key button data with button bits.
    pub const fn with_bits(bits: u32) -> Self {
        Self {
            raw: UnsafeCell::new(kbutton_t {
                down: [0; 2],
                state: 0,
            }),
            bits,
        }
    }

    fn get(&self) -> *mut kbutton_t {
        self.raw.get()
    }
}

#[derive(Copy, Clone)]
pub struct KeyButton {
    engine: ClientEngineRef,
    data: &'static KeyButtonData,
}

impl KeyButton {
    pub const fn new(engine: ClientEngineRef, data: &'static KeyButtonData) -> Self {
        Self { engine, data }
    }

    pub fn as_ptr(&self) -> *mut kbutton_t {
        self.data.get()
    }

    /// Button bits for this button.
    ///
    /// Used to set [usercmd_s.buttons](xash3d_ffi::common::usercmd_s.buttons).
    pub fn bits(&self) -> u32 {
        self.data.bits
    }

    pub fn state(&self) -> KeyState {
        let state = unsafe { (*self.as_ptr()).state };
        KeyState::from_bits_retain(state)
    }

    pub fn set_state(&self, state: KeyState) {
        unsafe {
            (*self.as_ptr()).state = state.bits();
        }
    }

    pub fn with_state(&self, map: impl FnOnce(KeyState) -> KeyState) {
        self.set_state(map(self.state()));
    }

    pub fn is_down(&self) -> bool {
        self.state().intersects(KeyState::DOWN)
    }

    pub fn is_up(&self) -> bool {
        !self.is_down()
    }

    pub fn is_impulse_down(&self) -> bool {
        self.state().intersects(KeyState::IMPULSE_DOWN)
    }

    pub fn is_impulse_up(&self) -> bool {
        self.state().intersects(KeyState::IMPULSE_UP)
    }

    pub fn is_down_or_impulse_down(&self) -> bool {
        self.state()
            .intersects(KeyState::DOWN | KeyState::IMPULSE_DOWN)
    }

    pub fn clear(&self) {
        unsafe {
            (*self.as_ptr()).down.fill(0);
        }
    }

    pub fn reset(&self) {
        unsafe { self.as_ptr().write_bytes(0, 1) }
    }

    pub fn key_down(&self) {
        let s = self.engine.cmd_argv(1);
        let k = if !s.is_empty() {
            s.to_str().ok().and_then(|s| s.parse().ok()).unwrap_or(0)
        } else {
            // typed manually at the console for continuous down
            -1
        };

        let down = unsafe { &mut (*self.as_ptr()).down };
        if !down.contains(&k) {
            if let Some(i) = down.iter_mut().find(|i| **i == 0) {
                *i = k;

                if !self.is_down() {
                    self.with_state(|state| state | KeyState::DOWN | KeyState::IMPULSE_DOWN);
                }
            }
        }
    }

    pub fn key_up(&self) {
        let s = self.engine.cmd_argv(1);
        if !s.is_empty() {
            let k = s.to_str().ok().and_then(|s| s.parse().ok()).unwrap_or(0);

            let down = unsafe { &mut (*self.as_ptr()).down };
            if let Some(i) = down.iter_mut().find(|i| **i == k) {
                *i = 0;

                if self.is_down() && !down.iter().any(|i| *i != 0) {
                    self.with_state(|state| {
                        state.difference(KeyState::DOWN).union(KeyState::IMPULSE_UP)
                    });
                }
            }
        } else {
            // typed manually at the console, assume for unsticking, so clear all
            self.clear();
            self.with_state(|state| state | KeyState::IMPULSE_UP);
        }
    }

    pub fn key_state(&self) -> f32 {
        let mut val = 0.0;
        let impulsedown = self.is_impulse_down();
        let impulseup = self.is_impulse_up();
        let down = self.is_down();

        if impulsedown && !impulseup && down {
            // pressed and held this frame?
            val = 0.5;
        }

        if impulseup && !impulsedown && down {
            // released this frame?
            val = 0.0;
        }

        if !impulsedown && !impulseup && down {
            // held the entire frame?
            val = 1.0;
        }

        if impulsedown && impulseup {
            if down {
                // released and re-pressed this frame
                val = 0.75;
            } else {
                // pressed and released this frame
                val = 0.25;
            }
        }

        // clear impulses
        self.with_state(|state| state & KeyState::DOWN);

        val
    }
}
