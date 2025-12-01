use core::{ffi::c_int, fmt::Write, num::NonZeroI32, ops::Deref, slice};

use alloc::vec::Vec;
use xash3d_shared::{
    color::RGB,
    csz::{CStrArray, CStrThin},
    ffi::{
        client::{HSPRITE, client_sprite_s},
        common::wrect_s,
    },
    str::{StringId, Strings},
};

use crate::prelude::*;

pub trait ClientSpriteExt {
    fn sprite(&self) -> &CStrThin;

    fn name(&self) -> &CStrThin;
}

impl ClientSpriteExt for client_sprite_s {
    fn sprite(&self) -> &CStrThin {
        unsafe { CStrThin::from_ptr(self.szSprite.as_ptr()) }
    }

    fn name(&self) -> &CStrThin {
        unsafe { CStrThin::from_ptr(self.szName.as_ptr()) }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpriteHandle {
    engine: ClientEngineRef,
    raw: NonZeroI32,
}

impl SpriteHandle {
    pub fn new(engine: ClientEngineRef, raw: HSPRITE) -> Option<Self> {
        NonZeroI32::new(raw).map(|raw| Self { engine, raw })
    }

    pub fn raw(&self) -> HSPRITE {
        self.raw.get()
    }

    pub fn frames(&self) -> i32 {
        self.engine.spr_frames(*self)
    }

    pub fn height(&self, frame: i32) -> i32 {
        self.engine.spr_height(*self, frame)
    }

    pub fn width(&self, frame: i32) -> i32 {
        self.engine.spr_width(*self, frame)
    }

    pub fn size(&self, frame: i32) -> (i32, i32) {
        self.engine.spr_size(*self, frame)
    }

    fn set(&self, color: RGB) {
        self.engine.spr_set(*self, color);
    }

    pub fn draw(&self, frame: i32, x: i32, y: i32, color: RGB) {
        self.set(color);
        self.engine.spr_draw(frame, x, y);
    }

    pub fn draw_rect(&self, frame: i32, x: i32, y: i32, color: RGB, rect: wrect_s) {
        self.set(color);
        self.engine.spr_draw_rect(frame, x, y, rect);
    }

    pub fn draw_holes(&self, frame: i32, x: i32, y: i32, color: RGB) {
        self.set(color);
        self.engine.spr_draw_holes(frame, x, y);
    }

    pub fn draw_holes_rect(&self, frame: i32, x: i32, y: i32, color: RGB, rect: wrect_s) {
        self.set(color);
        self.engine.spr_draw_holes_rect(frame, x, y, rect);
    }

    pub fn draw_additive(&self, frame: i32, x: i32, y: i32, color: RGB) {
        self.set(color);
        self.engine.spr_draw_additive(frame, x, y);
    }

    pub fn draw_additive_rect(&self, frame: i32, x: i32, y: i32, color: RGB, rect: wrect_s) {
        self.set(color);
        self.engine.spr_draw_additive_rect(frame, x, y, rect);
    }
}

#[derive(Copy, Clone)]
pub struct SpriteTexCoords {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl SpriteTexCoords {
    fn new(width: i32, height: i32, rect: &wrect_s) -> Self {
        let width = width as f32;
        let height = height as f32;
        Self {
            left: rect.left as f32 / width,
            right: rect.right as f32 / width,
            top: rect.top as f32 / height,
            bottom: rect.bottom as f32 / height,
        }
    }

    /// Returns texture coords clockwise from the top-left corner.
    pub fn cw(&self) -> [[f32; 2]; 4] {
        [
            [self.left, self.top],
            [self.right, self.top],
            [self.right, self.bottom],
            [self.left, self.bottom],
        ]
    }

    /// Returns texture coords counterclockwise from the top-left corner.
    pub fn ccw(&self) -> [[f32; 2]; 4] {
        [
            [self.left, self.top],
            [self.left, self.bottom],
            [self.right, self.bottom],
            [self.right, self.top],
        ]
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Sprite {
    handle: SpriteHandle,
    rect: wrect_s,
}

impl Sprite {
    pub fn new(handle: SpriteHandle, rect: wrect_s) -> Self {
        Sprite { handle, rect }
    }

    pub fn handle(&self) -> SpriteHandle {
        self.handle
    }

    pub fn frames(&self) -> i32 {
        self.handle.frames()
    }

    pub fn rect(&self) -> wrect_s {
        self.rect
    }

    pub fn width(&self) -> i32 {
        self.rect.right - self.rect.left
    }

    pub fn height(&self) -> i32 {
        self.rect.bottom - self.rect.top
    }

    pub fn size(&self) -> (i32, i32) {
        (self.width(), self.height())
    }

    pub fn tex_coords(&self, frame: i32) -> SpriteTexCoords {
        let width = self.handle().width(frame);
        let height = self.handle().height(frame);
        SpriteTexCoords::new(width, height, &self.rect())
    }

    pub fn draw(&self, frame: i32, x: i32, y: i32, color: RGB) {
        self.handle.draw_rect(frame, x, y, color, self.rect);
    }

    pub fn draw_holes(&self, frame: i32, x: i32, y: i32, color: RGB) {
        self.handle.draw_holes_rect(frame, x, y, color, self.rect);
    }

    pub fn draw_additive(&self, frame: i32, x: i32, y: i32, color: RGB) {
        self.handle
            .draw_additive_rect(frame, x, y, color, self.rect);
    }
}

#[derive(Default)]
pub struct DigitSprites {
    digits: [Option<Sprite>; 10],
    width: i32,
    height: i32,
}

impl DigitSprites {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_sprites(sprites: &Sprites) -> Self {
        let mut digits: [Option<Sprite>; 10] = [None; 10];
        let mut width = 0;
        let mut height = 0;
        let mut buf = CStrArray::<64>::new();
        for (i, digit) in digits.iter_mut().enumerate() {
            write!(buf.cursor(), "number_{i}").ok();
            let name = buf.as_thin();
            if let Some(sprite) = sprites.find(name) {
                *digit = Some(*sprite);
                if width == 0 {
                    width = sprite.width();
                    height = sprite.height();
                }
            }
        }
        Self {
            digits,
            width,
            height,
        }
    }

    pub fn get_by_char(&self, c: char) -> Option<&Sprite> {
        c.to_digit(10)
            .and_then(|i| self.digits[i as usize].as_ref())
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }
}

pub struct SpriteList {
    data: *const client_sprite_s,
    len: usize,
}

impl SpriteList {
    // pub fn empty() -> Self {
    //     Self {
    //         data: ptr::null(),
    //         len: 0,
    //     }
    // }

    pub(crate) fn new(data: *const client_sprite_s, len: usize) -> Self {
        Self { data, len }
    }

    pub fn as_slice(&self) -> &[client_sprite_s] {
        if !self.data.is_null() {
            unsafe { slice::from_raw_parts(self.data, self.len) }
        } else {
            &[]
        }
    }

    pub fn find(&self, name: &CStrThin, res: c_int) -> Option<&client_sprite_s> {
        self.as_slice()
            .iter()
            .filter(|i| i.iRes <= res && i.name() == name)
            .max_by_key(|i| i.iRes)
    }
}

impl Deref for SpriteList {
    type Target = [client_sprite_s];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

#[derive(Copy, Clone)]
pub struct NamedSprite {
    name: StringId,
    sprite: Sprite,
}

impl Deref for NamedSprite {
    type Target = Sprite;

    fn deref(&self) -> &Self::Target {
        &self.sprite
    }
}

pub struct Sprites {
    engine: ClientEngineRef,
    strings: Strings,
    sprites: Vec<NamedSprite>,
}

impl Sprites {
    pub fn new(engine: ClientEngineRef) -> Sprites {
        Self {
            engine,
            strings: Strings::new(),
            sprites: Vec::new(),
        }
    }

    pub fn load_from_list(engine: &ClientEngine, resolution: u32, list: &SpriteList) -> Self {
        let mut sprites = Self::new(engine.engine_ref());
        sprites.reload_from_list(resolution, list);
        sprites
    }

    pub fn load_from_file(
        engine: &ClientEngine,
        resolution: u32,
        path: impl AsRef<CStrThin>,
    ) -> Self {
        let mut sprites = Self::new(engine.engine_ref());
        sprites.reload_from_file(resolution, path);
        sprites
    }

    pub fn reload_from_list(&mut self, resolution: u32, list: &SpriteList) {
        self.strings.clear();
        self.sprites.clear();
        let engine = &*self.engine;
        let sprites = list
            .as_slice()
            .iter()
            .filter(|info| info.iRes as u32 == resolution)
            .filter_map(|info| {
                let handle = engine.spr_load(format_args!("sprites/{}.spr", info.sprite()))?;
                Some((info, handle))
            })
            .map(|(info, handle)| NamedSprite {
                name: self.strings.from_thin(info.name()),
                sprite: Sprite::new(handle, info.rc),
            });
        self.sprites.extend(sprites);
        self.strings.shrink_to_fit();
        self.sprites.shrink_to_fit();
    }

    pub fn reload_from_file(&mut self, resolution: u32, path: impl AsRef<CStrThin>) {
        let list = self.engine.spr_get_list(path.as_ref());
        self.reload_from_list(resolution, &list);
    }

    fn name_for(&self, sprite: &NamedSprite) -> &CStrThin {
        self.strings.get(sprite.name)
    }

    fn find_index_impl(&self, name: &CStrThin) -> Option<usize> {
        self.iter().position(|i| self.name_for(i) == name)
    }

    pub fn find_index(&self, name: impl AsRef<CStrThin>) -> Option<usize> {
        self.find_index_impl(name.as_ref())
    }

    fn find_impl(&self, name: &CStrThin) -> Option<&Sprite> {
        self.iter()
            .find(|i| self.name_for(i) == name)
            .map(|i| &i.sprite)
    }

    pub fn find(&self, name: impl AsRef<CStrThin>) -> Option<&Sprite> {
        self.find_impl(name.as_ref())
    }
}

impl Deref for Sprites {
    type Target = [NamedSprite];

    fn deref(&self) -> &Self::Target {
        &self.sprites
    }
}
