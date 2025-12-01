pub mod net;
pub mod tri;

use core::{
    cmp::Ordering,
    ffi::{c_char, c_int},
    fmt::{self, Write},
    hash,
    marker::PhantomData,
    ops::Deref,
    time::Duration,
};

use csz::{CStrArray, CStrThin};
use xash3d_ffi::common::{cvar_s, vec3_t};

use crate::{
    color::RGB,
    cvar::{Cvar, GetCvar, SetCvar},
    export::UnsyncGlobal,
    file::File,
    math::fabsf,
    str::{AsCStrPtr, ToEngineStr},
};

/// A wrapper for an engine struct that can not be send or called from other threads.
pub struct EngineRef<T> {
    phantom: PhantomData<*const T>,
}

impl<T> EngineRef<T> {
    /// Creates a new `EngineRef<T>`.
    ///
    /// # Safety
    ///
    /// Must be called only in the engine thread and `T` must be initialized.
    pub unsafe fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<T: UnsyncGlobal> Deref for EngineRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { T::global_assume_init_ref() }
    }
}

impl<T> Copy for EngineRef<T> {}

impl<T> Clone for EngineRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for EngineRef<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for EngineRef<T> {}

impl<T> PartialOrd for EngineRef<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for EngineRef<T> {
    fn cmp(&self, _: &Self) -> Ordering {
        Ordering::Equal
    }
}

impl<T> fmt::Debug for EngineRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineRef").finish()
    }
}

impl<T> hash::Hash for EngineRef<T> {
    fn hash<H: hash::Hasher>(&self, _: &mut H) {}
}

/// The error type which is returned if a buffer capacity is no sufficient to hold all data.
#[derive(Debug)]
pub struct BufferError;

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt("buffer capacity is too low", f)
    }
}

/// Engine API to read and modify console variables.
pub trait EngineCvar: Sized {
    #[doc(hidden)]
    fn fn_get_cvar_float(&self) -> unsafe extern "C" fn(name: *const c_char) -> f32;

    #[doc(hidden)]
    fn fn_set_cvar_float(&self) -> unsafe extern "C" fn(name: *const c_char, value: f32);

    #[doc(hidden)]
    fn fn_get_cvar_string(&self) -> unsafe extern "C" fn(name: *const c_char) -> *const c_char;

    #[doc(hidden)]
    fn fn_set_cvar_string(&self)
    -> unsafe extern "C" fn(name: *const c_char, value: *const c_char);

    #[doc(hidden)]
    fn fn_direct_set_cvar_string(
        &self,
    ) -> Option<unsafe extern "C" fn(var: *mut cvar_s, value: *const c_char)> {
        None
    }

    fn get_cvar_float(&self, name: impl ToEngineStr) -> f32 {
        let name = name.to_engine_str();
        unsafe { self.fn_get_cvar_float()(name.as_ptr()) }
    }

    fn set_cvar_float(&self, name: impl ToEngineStr, value: f32) {
        let name = name.to_engine_str();
        unsafe { self.fn_set_cvar_float()(name.as_ptr(), value) }
    }

    fn get_cvar_string(&self, name: impl ToEngineStr) -> &CStrThin {
        let name = name.to_engine_str();
        // FIXME: The lifetime of the returned string is valid only until the cvar is modified.
        let ptr = unsafe { self.fn_get_cvar_string()(name.as_ptr()) };
        // SAFETY: the engine returns an empty string if cvar is not found
        unsafe { CStrThin::from_ptr(ptr) }
    }

    fn set_cvar_string(&self, name: impl ToEngineStr, value: impl ToEngineStr) {
        let name = name.to_engine_str();
        let value = value.to_engine_str();
        unsafe { self.fn_set_cvar_string()(name.as_ptr(), value.as_ptr()) }
    }

    fn direct_set_cvar_float<T>(&self, cvar: &Cvar<Self, T>, value: f32) {
        if let Some(f) = self.fn_direct_set_cvar_string() {
            let mut buf = CStrArray::<64>::new();
            if fabsf(value - value as i32 as f32) < 0.000001 {
                write!(buf.cursor(), "{}", value as i32).unwrap();
            } else {
                write!(buf.cursor(), "{}", value).unwrap();
            }
            unsafe { (f)(cvar.as_ptr(), buf.as_ptr()) }
        } else {
            self.set_cvar_float(cvar.name(), value);
        }
    }

    fn direct_set_cvar_string<T>(&self, cvar: &Cvar<Self, T>, value: impl ToEngineStr) {
        let value = value.to_engine_str();
        if let Some(f) = self.fn_direct_set_cvar_string() {
            unsafe { (f)(cvar.as_ptr(), value.as_ptr()) }
        } else {
            self.set_cvar_string(cvar.name(), value.as_ref());
        }
    }

    fn get_cvar<'a, T: GetCvar<'a>>(&'a self, name: impl ToEngineStr) -> T {
        T::get_cvar(self, name)
    }

    fn set_cvar<T: SetCvar>(&self, name: impl ToEngineStr, value: T) {
        T::set_cvar(self, name, value)
    }

    fn direct_set_cvar<T: SetCvar>(&self, cvar: &Cvar<Self, T>, value: T) {
        T::direct_set_cvar(self, cvar, value)
    }
}

/// Engine API to generate random numbers.
pub trait EngineRng {
    #[doc(hidden)]
    fn fn_random_float(&self) -> unsafe extern "C" fn(min: f32, max: f32) -> f32;

    #[doc(hidden)]
    fn fn_random_int(&self) -> unsafe extern "C" fn(min: c_int, max: c_int) -> c_int;

    fn random_float(&self, min: f32, max: f32) -> f32 {
        unsafe { self.fn_random_float()(min, max) }
    }

    fn random_int(&self, min: c_int, max: c_int) -> c_int {
        assert!(min >= 0, "min must be greater than or equal to zero");
        assert!(min <= max, "min must be less than or equal to max");
        unsafe { self.fn_random_int()(min, max) }
    }

    fn random_bool(&self) -> bool {
        self.random_int(0, 1) != 0
    }

    /// Returns a vector with random elements in range from `min` to `max`.
    fn random_vec3(&self, min: f32, max: f32) -> vec3_t {
        vec3_t::new(
            self.random_float(min, max),
            self.random_float(min, max),
            self.random_float(min, max),
        )
    }

    /// Returns a random element from a given slice if the slice is not empty.
    ///
    /// # Panics
    ///
    /// Panics if the slice length is greater than [i32::MAX].
    fn random_element<'a, T>(&self, slice: &'a [T]) -> Option<&'a T> {
        let max = slice
            .len()
            .checked_sub(1)?
            .try_into()
            .expect("length must be less than or equal to i32::MAX");
        slice.get(self.random_int(0, max) as usize)
    }
}

/// Engine API to print messages to the console.
pub trait EngineConsole {
    fn console_print(&self, msg: impl ToEngineStr);
}

#[derive(Debug)]
pub struct AddCmdError;

impl fmt::Display for AddCmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "failed to add a console command".fmt(f)
    }
}

/// Engine API to register console commands and access arguments.
pub trait EngineCmd {
    #[doc(hidden)]
    fn fn_cmd_argc(&self) -> unsafe extern "C" fn() -> c_int;

    #[doc(hidden)]
    fn fn_cmd_argv(&self) -> unsafe extern "C" fn(argc: c_int) -> *const c_char;

    fn add_command(
        &self,
        name: impl ToEngineStr,
        func: unsafe extern "C" fn(),
    ) -> Result<(), AddCmdError>;

    fn cmd_argc(&self) -> usize {
        unsafe { self.fn_cmd_argc()() as usize }
    }

    fn cmd_argv(&self, index: usize) -> &CStrThin {
        let ptr = unsafe { self.fn_cmd_argv()(index as c_int) };
        // SAFETY: the engine returns an empty string if index is greater than
        // arguments count
        unsafe { CStrThin::from_ptr(ptr) }
    }

    /// Returns an iterator over command arguments.
    fn cmd_args(&self) -> impl Iterator<Item = &CStrThin> {
        (0..self.cmd_argc()).map(|i| self.cmd_argv(i))
    }
}

/// Engine API to access a raw command arguments string.
pub trait EngineCmdArgsRaw {
    #[doc(hidden)]
    fn fn_cmd_args_raw(&self) -> unsafe extern "C" fn() -> *const c_char;

    /// Returns a raw command arguments string without command name.
    fn cmd_args_raw(&self) -> Option<&CStrThin> {
        let ptr = unsafe { self.fn_cmd_args_raw()() };
        if !ptr.is_null() {
            Some(unsafe { CStrThin::from_ptr(ptr) })
        } else {
            None
        }
    }
}

/// Engine API to access the duration elapsed from the engine startup.
pub trait EngineSystemTime {
    /// Returns the number of seconds elapsed from the engine startup to the current time.
    fn system_time_f64(&self) -> f64;

    /// Returns the duration elapsed from the engine startup to the current time.
    fn system_time(&self) -> Duration {
        Duration::from_secs_f64(self.system_time_f64())
    }
}

/// Engine API to draw a text on the screen with a console font.
pub trait EngineDrawConsoleString {
    /// Sets the color for drawing text.
    fn set_text_color(&self, color: impl Into<RGB>);

    /// Returns the width and height of the text on the screen in pixels.
    fn console_string_size(&self, text: impl ToEngineStr) -> (c_int, c_int);

    /// Returns the width of the text on the screen in pixels.
    fn console_string_width(&self, text: impl ToEngineStr) -> c_int {
        let (width, _) = self.console_string_size(text);
        width
    }

    /// Returns the height of the text on the screen in pixels.
    fn console_string_height(&self, text: impl ToEngineStr) -> c_int {
        let (_, height) = self.console_string_size(text);
        height
    }

    /// Draw the text at given coordinates.
    ///
    /// Returns the x coordinate after the drawn text.
    fn draw_console_string(&self, x: c_int, y: c_int, text: impl ToEngineStr) -> c_int;
}

/// An error returned from the [EngineFile::load_file] function.
pub struct LoadFileError(());

impl fmt::Debug for LoadFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("LoadFileError").finish()
    }
}

impl fmt::Display for LoadFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("failed to load a file")
    }
}

pub trait EngineFile: UnsyncGlobal {
    #[doc(hidden)]
    fn load_file_raw(&self, path: &CStrThin, len: &mut i32) -> *mut u8;

    /// Free a file.
    ///
    /// # Safety
    ///
    /// The file must be allocated with [load_file](Self::load_file) function.
    unsafe fn free_file_raw(&self, file: *mut u8);

    /// Load a file by the given path.
    fn load_file(&self, path: impl ToEngineStr) -> Result<File<Self>, LoadFileError> {
        let path = path.to_engine_str();
        let mut len = 0;
        let data = self.load_file_raw(path.as_ref(), &mut len);
        if !data.is_null() {
            Ok(unsafe { File::new(data.cast(), len as usize) })
        } else {
            Err(LoadFileError(()))
        }
    }
}
