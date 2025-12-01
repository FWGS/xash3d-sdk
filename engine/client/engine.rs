pub mod demo;
pub mod efx;
pub mod event;

pub use xash3d_shared::engine::tri;

use core::{
    ffi::{c_char, c_int, c_void},
    mem::{self, MaybeUninit},
    ptr,
};

use xash3d_shared::{
    csz::{CStrSlice, CStrThin},
    cvar::CvarFlags,
    engine::net::NetApi,
    export::impl_unsync_global,
    ffi::{
        client::{cl_enginefuncs_s, client_textmessage_s, hud_player_info_s},
        common::{cl_entity_s, event_args_s, model_s, vec3_t, wrect_s},
    },
    str::{AsCStrPtr, ToEngineStr},
    utils::cstr_or_none,
};

use crate::{
    color::{RGB, RGBA},
    cvar::Cvar,
    engine::{demo::DemoApi, efx::EfxApi, event::EventApi, tri::TriangleApi},
    global_state::GlobalStateRef,
    screen::ScreenInfo,
    sprite::{SpriteHandle, SpriteList},
};

#[allow(deprecated)]
use crate::cvar::{CVarFlags, CVarPtr};

pub use xash3d_shared::engine::{AddCmdError, BufferError, EngineRef, net};

pub(crate) mod prelude {
    pub use xash3d_shared::engine::EngineCmd;
    pub use xash3d_shared::engine::EngineConsole;
    pub use xash3d_shared::engine::EngineCvar;
    pub use xash3d_shared::engine::EngineDrawConsoleString;
    pub use xash3d_shared::engine::EngineFile;
    pub use xash3d_shared::engine::EngineRng;
    pub use xash3d_shared::engine::EngineSystemTime;
    pub use xash3d_shared::engine::net::EngineNet;
}

pub use self::prelude::*;

pub use xash3d_shared::engine::LoadFileError;

pub type ClientEngineRef = EngineRef<ClientEngine>;

pub type UserMsgHookFn =
    Option<unsafe extern "C" fn(name: *const c_char, size: c_int, buf: *mut c_void) -> c_int>;

pub struct PlayerInfo {
    raw: hud_player_info_s,
}

impl PlayerInfo {
    pub fn from_raw(raw: hud_player_info_s) -> Option<PlayerInfo> {
        if !raw.name.is_null() {
            assert!(!raw.model.is_null(), "model must not be null");
            Some(Self { raw })
        } else {
            None
        }
    }

    pub fn as_raw(&self) -> &hud_player_info_s {
        &self.raw
    }

    pub fn name(&self) -> &CStrThin {
        // SAFETY: checked in from_raw
        unsafe { CStrThin::from_ptr(self.raw.name) }
    }

    pub fn ping(&self) -> u32 {
        self.raw.ping as u32
    }

    pub fn packet_loss_percent(&self) -> u8 {
        self.raw.packetloss
    }

    pub fn is_this_player(&self) -> bool {
        self.raw.thisplayer != 0
    }

    pub fn is_spectator(&self) -> bool {
        self.raw.spectator != 0
    }

    pub fn model(&self) -> &CStrThin {
        // SAFETY: the pointer is non-null
        unsafe { CStrThin::from_ptr(self.raw.model) }
    }

    pub fn top_color(&self) -> i16 {
        self.raw.topcolor
    }

    pub fn bottom_color(&self) -> i16 {
        self.raw.bottomcolor
    }

    pub fn steam_id(&self) -> u64 {
        self.raw.m_nSteamID
    }
}

pub struct ClientEngine {
    raw: cl_enginefuncs_s,
    efx_api: EfxApi,
    event_api: EventApi,
    demo_api: DemoApi,
    net_api: NetApi,
}

impl_unsync_global!(ClientEngine);

macro_rules! unwrap {
    ($self:expr, $name:ident) => {
        match $self.raw.$name {
            Some(func) => func,
            None => panic!("cl_enginefuncs_s.{} is null", stringify!($name)),
        }
    };
}

impl ClientEngine {
    pub(crate) fn new(raw: &cl_enginefuncs_s) -> Self {
        Self {
            raw: *raw,
            efx_api: EfxApi::new(raw.pEfxAPI),
            event_api: EventApi::new(raw.pEventAPI),
            demo_api: DemoApi::new(raw.pDemoAPI),
            net_api: NetApi::new(raw.pNetAPI),
        }
    }

    pub fn raw(&self) -> &cl_enginefuncs_s {
        &self.raw
    }

    pub fn engine_ref(&self) -> ClientEngineRef {
        // SAFETY: we are in the game thread
        unsafe { ClientEngineRef::new() }
    }

    pub fn global_state_ref(&self) -> GlobalStateRef {
        // SAFETY: we are in the game thread
        unsafe { GlobalStateRef::new() }
    }

    /// Returns a shared reference to the triangle API, or returns `None` if not supported by the
    /// engine.
    pub fn try_tri_api(&self) -> Option<&TriangleApi> {
        if !self.raw.pTriAPI.is_null() {
            Some(TriangleApi::new(unsafe { &*self.raw.pTriAPI }))
        } else {
            None
        }
    }

    /// Returns a shared reference to the triangle API.
    ///
    /// # Panics
    ///
    /// Panics if the engine does not support the tirangle API.
    pub fn tri_api(&self) -> &TriangleApi {
        self.try_tri_api().expect("triangle api must be present")
    }

    pub fn efx_api(&self) -> &EfxApi {
        &self.efx_api
    }

    pub fn event_api(&self) -> &EventApi {
        &self.event_api
    }

    pub fn demo_api(&self) -> &DemoApi {
        &self.demo_api
    }

    pub fn spr_load(&self, pic_name: impl ToEngineStr) -> Option<SpriteHandle> {
        let pic_name = pic_name.to_engine_str();
        let raw = unsafe { unwrap!(self, pfnSPR_Load)(pic_name.as_ptr()) };
        SpriteHandle::new(self.engine_ref(), raw)
    }

    pub fn spr_frames(&self, pic: SpriteHandle) -> c_int {
        unsafe { unwrap!(self, pfnSPR_Frames)(pic.raw()) }
    }

    pub fn spr_height(&self, pic: SpriteHandle, frame: c_int) -> c_int {
        unsafe { unwrap!(self, pfnSPR_Height)(pic.raw(), frame) }
    }

    pub fn spr_width(&self, pic: SpriteHandle, frame: c_int) -> c_int {
        unsafe { unwrap!(self, pfnSPR_Width)(pic.raw(), frame) }
    }

    pub fn spr_size(&self, pic: SpriteHandle, frame: c_int) -> (c_int, c_int) {
        let w = self.spr_width(pic, frame);
        let h = self.spr_height(pic, frame);
        (w, h)
    }

    pub fn spr_set(&self, pic: SpriteHandle, color: RGB) {
        let [r, g, b] = color.into();
        unsafe { unwrap!(self, pfnSPR_Set)(pic.raw(), r, g, b) }
    }

    pub fn spr_draw(&self, frame: c_int, x: c_int, y: c_int) {
        unsafe { unwrap!(self, pfnSPR_Draw)(frame, x, y, ptr::null()) }
    }

    pub fn spr_draw_rect(&self, frame: c_int, x: c_int, y: c_int, rect: wrect_s) {
        unsafe { unwrap!(self, pfnSPR_Draw)(frame, x, y, &rect) }
    }

    pub fn spr_draw_holes(&self, frame: c_int, x: c_int, y: c_int) {
        unsafe { unwrap!(self, pfnSPR_DrawHoles)(frame, x, y, ptr::null()) }
    }

    pub fn spr_draw_holes_rect(&self, frame: c_int, x: c_int, y: c_int, rect: wrect_s) {
        unsafe { unwrap!(self, pfnSPR_DrawHoles)(frame, x, y, &rect) }
    }

    pub fn spr_draw_additive(&self, frame: c_int, x: c_int, y: c_int) {
        unsafe { unwrap!(self, pfnSPR_DrawAdditive)(frame, x, y, ptr::null()) }
    }

    pub fn spr_draw_additive_rect(&self, frame: c_int, x: c_int, y: c_int, rect: wrect_s) {
        unsafe { unwrap!(self, pfnSPR_DrawAdditive)(frame, x, y, &rect) }
    }

    pub fn spr_scissor_enable(&self, x: c_int, y: c_int, width: c_int, height: c_int) {
        unsafe { unwrap!(self, pfnSPR_EnableScissor)(x, y, width, height) }
    }

    pub fn spr_scissor_disable(&self) {
        unsafe { unwrap!(self, pfnSPR_DisableScissor)() }
    }

    // TODO: spr_get_list: return result?
    pub fn spr_get_list(&self, path: impl ToEngineStr) -> SpriteList {
        let path = path.to_engine_str();
        unsafe {
            let mut len = MaybeUninit::uninit();
            // FIXME: ffi: why psz is mutable?
            let data = unwrap!(self, pfnSPR_GetList)(path.as_ptr().cast_mut(), len.as_mut_ptr());
            SpriteList::new(data, len.assume_init() as usize)
        }
    }

    pub fn fill_rgba(&self, x: c_int, y: c_int, width: c_int, height: c_int, color: RGBA) {
        unsafe {
            let [r, g, b, a] = color.into();
            unwrap!(self, pfnFillRGBA)(x, y, width, height, r, g, b, a)
        }
    }

    pub fn screen_info(&self) -> ScreenInfo {
        let mut info = ScreenInfo::default();
        if unsafe { unwrap!(self, pfnGetScreenInfo)(info.as_mut()) } != 1 {
            panic!("screen_info: unexpected result from the engine");
        }
        info
    }

    pub fn unset_crosshair(&self) {
        let rect = unsafe { mem::zeroed() };
        unsafe { unwrap!(self, pfnSetCrosshair)(0, rect, 0, 0, 0) }
    }

    pub fn set_crosshair(&self, sprite: SpriteHandle, rect: wrect_s, color: RGB) {
        let [r, g, b] = color.into();
        unsafe { unwrap!(self, pfnSetCrosshair)(sprite.raw(), rect, r, g, b) }
    }

    #[deprecated]
    #[allow(deprecated)]
    pub fn register_variable(
        &self,
        name: impl ToEngineStr,
        value: impl ToEngineStr,
        flags: CVarFlags,
    ) -> Option<CVarPtr> {
        let name = name.to_engine_str();
        let value = value.to_engine_str();
        unsafe {
            let raw = unwrap!(self, pfnRegisterVariable)(
                name.as_ptr(),
                value.as_ptr(),
                flags.bits() as c_int,
            );
            if !raw.is_null() {
                Some(CVarPtr::from_ptr(raw.cast()))
            } else {
                None
            }
        }
    }

    /// Register a console variable.
    pub fn register_cvar(
        &self,
        name: impl ToEngineStr,
        value: impl ToEngineStr,
        flags: CvarFlags,
    ) -> bool {
        self.create_cvar::<f32>(name, value, flags).is_some()
    }

    /// Creates a console variable.
    pub fn create_cvar<T>(
        &self,
        name: impl ToEngineStr,
        value: impl ToEngineStr,
        flags: CvarFlags,
    ) -> Option<Cvar<T>> {
        let name = name.to_engine_str();
        let value = value.to_engine_str();
        let ptr = unsafe {
            unwrap!(self, pfnRegisterVariable)(name.as_ptr(), value.as_ptr(), flags.bits() as i32)
        };
        unsafe { Cvar::new(self.engine_ref(), ptr) }
    }

    /// Searches for a cvar with the given name.
    pub fn find_cvar<T>(&self, name: impl ToEngineStr) -> Option<Cvar<T>> {
        let name = name.to_engine_str();
        let ptr = unsafe { unwrap!(self, pfnGetCvarPointer)(name.as_ptr()) };
        unsafe { Cvar::new(self.engine_ref(), ptr) }
    }

    pub fn hook_user_msg(&self, name: impl ToEngineStr, func: UserMsgHookFn) -> c_int {
        let name = name.to_engine_str();
        unsafe { unwrap!(self, pfnHookUserMsg)(name.as_ptr(), func) }
    }

    pub fn server_cmd(&self, cmd: impl ToEngineStr) -> c_int {
        let cmd = cmd.to_engine_str();
        unsafe { unwrap!(self, pfnServerCmd)(cmd.as_ptr()) }
    }

    pub fn client_cmd(&self, cmd: impl ToEngineStr) -> c_int {
        let cmd = cmd.to_engine_str();
        unsafe { unwrap!(self, pfnClientCmd)(cmd.as_ptr()) }
    }

    pub fn get_player_info(&self, entity: c_int) -> Option<PlayerInfo> {
        unsafe {
            let mut info = MaybeUninit::uninit();
            unwrap!(self, pfnGetPlayerInfo)(entity, info.as_mut_ptr());
            PlayerInfo::from_raw(info.assume_init())
        }
    }

    pub fn play_sound_by_name(&self, name: impl ToEngineStr, vol: f32) {
        let name = name.to_engine_str();
        unsafe { unwrap!(self, pfnPlaySoundByName)(name.as_ptr(), vol) }
    }

    // pub pfnPlaySoundByIndex: Option<unsafe extern "C" fn(iSound: c_int, volume: f32)>,
    // pub pfnAngleVectors: Option<
    //     unsafe extern "C" fn(
    //         vecAngles: *const f32,
    //         forward: *mut f32,
    //         right: *mut f32,
    //         up: *mut f32,
    //     ),
    // >,

    pub fn text_message_get(&self, msg: impl ToEngineStr) -> Option<&'static client_textmessage_s> {
        let msg = msg.to_engine_str();
        unsafe {
            let ret = unwrap!(self, pfnTextMessageGet)(msg.as_ptr());
            if !ret.is_null() && !(*ret).pMessage.is_null() {
                Some(&*ret)
            } else {
                None
            }
        }
    }

    pub fn draw_character(&self, x: c_int, y: c_int, number: c_int, color: RGB) -> c_int {
        let [r, g, b] = color.into();
        unsafe { unwrap!(self, pfnDrawCharacter)(x, y, number, r, g, b) }
    }

    /// Sets the color of the console text for drawing.
    ///
    /// Values must be in range [0..1] inclusive.
    pub fn set_text_color_f32(&self, r: f32, g: f32, b: f32) {
        unsafe { unwrap!(self, pfnDrawSetTextColor)(r, g, b) }
    }

    // pub pfnCenterPrint: Option<unsafe extern "C" fn(string: *const c_char)>,

    pub fn get_window_center_x(&self) -> c_int {
        unsafe { unwrap!(self, GetWindowCenterX)() }
    }

    pub fn get_window_center_y(&self) -> c_int {
        unsafe { unwrap!(self, GetWindowCenterY)() }
    }

    pub fn get_window_center(&self) -> (c_int, c_int) {
        (self.get_window_center_x(), self.get_window_center_y())
    }

    pub fn get_view_angles(&self) -> vec3_t {
        unsafe {
            let mut ret = MaybeUninit::<[f32; 3]>::uninit();
            unwrap!(self, GetViewAngles)(ret.as_mut_ptr().cast());
            ret.assume_init().into()
        }
    }

    pub fn set_view_angles(&self, mut angles: vec3_t) {
        // FIXME: ffi: why arg1 is mutable?
        unsafe { unwrap!(self, SetViewAngles)(angles.as_mut().as_mut_ptr()) }
    }

    pub fn get_max_clients(&self) -> c_int {
        unsafe { unwrap!(self, GetMaxClients)() }
    }

    // pub Con_Printf: Option<unsafe extern "C" fn(fmt: *const c_char, ...)>,
    // pub Con_DPrintf: Option<unsafe extern "C" fn(fmt: *const c_char, ...)>,
    // pub Con_NPrintf: Option<unsafe extern "C" fn(pos: c_int, fmt: *const c_char, ...)>,
    // pub Con_NXPrintf:
    //     Option<unsafe extern "C" fn(info: *mut con_nprint_s, fmt: *const c_char, ...)>,
    // pub PhysInfo_ValueForKey: Option<unsafe extern "C" fn(key: *const c_char) -> *const c_char>,
    // pub ServerInfo_ValueForKey: Option<unsafe extern "C" fn(key: *const c_char) -> *const c_char>,

    pub fn get_client_max_speed(&self) -> f32 {
        unsafe { unwrap!(self, GetClientMaxspeed)() }
    }

    pub fn check_parm(&self, parm: impl ToEngineStr) -> bool {
        let parm = parm.to_engine_str();
        // FIXME: ffi: why parm is mutable?
        unsafe { unwrap!(self, CheckParm)(parm.as_ptr().cast_mut(), ptr::null_mut()) != 0 }
    }

    pub fn get_parm(&self, parm: impl ToEngineStr) -> Option<&CStrThin> {
        let parm = parm.to_engine_str();
        let mut next = ptr::null_mut();
        unsafe {
            // FIXME: ffi: why parm is mutable?
            unwrap!(self, CheckParm)(parm.as_ptr().cast_mut(), &mut next);
            cstr_or_none(next)
        }
    }

    pub fn key_event(&self, key: i32, down: bool) {
        unsafe {
            unwrap!(self, Key_Event)(key as c_int, down as c_int);
        }
    }

    pub fn get_mouse_position(&self) -> (c_int, c_int) {
        unsafe {
            let mut x = 0;
            let mut y = 0;
            unwrap!(self, GetMousePosition)(&mut x, &mut y);
            (x, y)
        }
    }

    pub fn is_no_clipping(&self) -> bool {
        unsafe { unwrap!(self, IsNoClipping)() != 0 }
    }

    /// Returns the entity of local player model.
    ///
    /// # SAFETY
    ///
    /// Never returns a null pointer.
    pub fn get_local_player(&self) -> *mut cl_entity_s {
        let ent = unsafe { unwrap!(self, GetLocalPlayer)() };
        assert!(!ent.is_null());
        ent
    }

    /// Returns the entity of weapon model.
    ///
    /// # SAFETY
    ///
    /// Never returns a null pointer.
    pub fn get_view_entity(&self) -> *mut cl_entity_s {
        let ent = unsafe { unwrap!(self, GetViewModel)() };
        assert!(!ent.is_null());
        ent
    }

    pub fn get_entity_by_index(&self, index: c_int) -> *mut cl_entity_s {
        unsafe { unwrap!(self, GetEntityByIndex)(index) }
    }

    pub fn get_client_time(&self) -> f32 {
        unsafe { unwrap!(self, GetClientTime)() }
    }

    pub fn calc_shake(&self) {
        unsafe { unwrap!(self, V_CalcShake)() }
    }

    pub fn apply_shake(&self, origin: &mut vec3_t, angles: &mut vec3_t, factor: f32) {
        let origin = origin.as_mut().as_mut_ptr();
        let angles = angles.as_mut().as_mut_ptr();
        unsafe { unwrap!(self, V_ApplyShake)(origin, angles, factor) }
    }

    pub fn pm_point_contents(&self, point: vec3_t) -> (c_int, c_int) {
        unsafe {
            let mut truecont = MaybeUninit::uninit();
            let cont =
                unwrap!(self, PM_PointContents)(point.as_ref().as_ptr(), truecont.as_mut_ptr());
            (cont, truecont.assume_init())
        }
    }

    pub fn pm_water_entity(&self, point: vec3_t) -> c_int {
        unsafe { unwrap!(self, PM_WaterEntity)(point.as_ref().as_ptr()) }
    }

    // pub PM_TraceLine: Option<
    //     unsafe extern "C" fn(
    //         start: *mut f32,
    //         end: *mut f32,
    //         flags: c_int,
    //         usehull: c_int,
    //         ignore_pe: c_int,
    //     ) -> *mut pmtrace_s,
    // >,
    // pub CL_LoadModel:
    //     Option<unsafe extern "C" fn(modelname: *const c_char, index: *mut c_int) -> *mut model_s>,
    // pub CL_CreateVisibleEntity:
    //     Option<unsafe extern "C" fn(type_: c_int, ent: *mut cl_entity_s) -> c_int>,

    pub fn get_sprite_model_raw(&self, sprite: SpriteHandle) -> Option<&model_s> {
        let model = unsafe { unwrap!(self, GetSpritePointer)(sprite.raw()) };
        if !model.is_null() {
            Some(unsafe { &*model })
        } else {
            None
        }
    }

    // pub pfnPlaySoundByNameAtLocation:
    //     Option<unsafe extern "C" fn(szSound: *mut c_char, volume: f32, origin: *mut f32)>,
    // pub pfnPrecacheEvent:
    //     Option<unsafe extern "C" fn(type_: c_int, psz: *const c_char) -> c_ushort>,
    // pub pfnPlaybackEvent: Option<
    //     unsafe extern "C" fn(
    //         flags: c_int,
    //         pInvoker: *const edict_s,
    //         eventindex: c_ushort,
    //         delay: f32,
    //         origin: *mut f32,
    //         angles: *mut f32,
    //         fparam1: f32,
    //         fparam2: f32,
    //         iparam1: c_int,
    //         iparam2: c_int,
    //         bparam1: c_int,
    //         bparam2: c_int,
    //     ),
    // >,
    // pub pfnWeaponAnim: Option<unsafe extern "C" fn(iAnim: c_int, body: c_int)>,

    // TODO: move to EngineRng
    pub fn rand(&self) -> c_int {
        self.random_int(0, c_int::MAX)
    }

    pub fn hook_event(
        &self,
        name: impl ToEngineStr,
        event: Option<unsafe extern "C" fn(args: *mut event_args_s)>,
    ) {
        let name = name.to_engine_str();
        unsafe { unwrap!(self, pfnHookEvent)(name.as_ptr(), event) }
    }

    // pub Con_IsVisible: Option<unsafe extern "C" fn() -> c_int>,
    // pub pfnGetGameDirectory: Option<unsafe extern "C" fn() -> *const c_char>,

    #[deprecated]
    #[allow(deprecated)]
    pub fn get_cvar(&self, name: impl ToEngineStr) -> CVarPtr {
        let name = name.to_engine_str();
        let ptr = unsafe { unwrap!(self, pfnGetCvarPointer)(name.as_ptr()) };
        CVarPtr::from_ptr(ptr.cast())
    }

    pub fn key_lookup_binding<'a>(
        &self,
        binding: impl ToEngineStr,
        buffer: &'a mut CStrSlice,
    ) -> Result<&'a CStrThin, BufferError> {
        let binding = binding.to_engine_str();
        let s = unsafe { unwrap!(self, Key_LookupBinding)(binding.as_ptr()) };
        assert!(!s.is_null());
        let s = unsafe { CStrThin::from_ptr(s) }.to_bytes();
        buffer.cursor().write_bytes(s).map_err(|_| BufferError)?;
        Ok(buffer.as_thin())
    }

    // pub pfnGetLevelName: Option<unsafe extern "C" fn() -> *const c_char>,
    // pub pfnGetScreenFade: Option<unsafe extern "C" fn(fade: *mut screenfade_s)>,
    // pub pfnSetScreenFade: Option<unsafe extern "C" fn(fade: *mut screenfade_s)>,
    // pub VGui_GetPanel: Option<unsafe extern "C" fn() -> *mut c_void>,
    // pub VGui_ViewportPaintBackground: Option<unsafe extern "C" fn(extents: *mut [c_int; 4usize])>,
    // pub COM_LoadFile: Option<
    //     unsafe extern "C" fn(path: *const c_char, usehunk: c_int, pLength: *mut c_int) -> *mut byte,
    // >,
    // pub COM_ParseFile:
    //     Option<unsafe extern "C" fn(data: *mut c_char, token: *mut c_char) -> *mut c_char>,
    // pub COM_FreeFile: Option<unsafe extern "C" fn(buffer: *mut c_void)>,
    // pub pVoiceTweak: *mut IVoiceTweak_s,

    pub fn is_spectator_only(&self) -> bool {
        unsafe { unwrap!(self, IsSpectateOnly)() != 0 }
    }

    // pub LoadMapSprite: Option<unsafe extern "C" fn(filename: *const c_char) -> *mut model_s>,
    // pub COM_AddAppDirectoryToSearchPath:
    //     Option<unsafe extern "C" fn(pszBaseDir: *const c_char, appName: *const c_char)>,
    // pub COM_ExpandFilename: Option<
    //     unsafe extern "C" fn(
    //         fileName: *const c_char,
    //         nameOutBuffer: *mut c_char,
    //         nameOutBufferSize: c_int,
    //     ) -> c_int,
    // >,
    // pub PlayerInfo_ValueForKey:
    //     Option<unsafe extern "C" fn(playerNum: c_int, key: *const c_char) -> *const c_char>,
    // pub PlayerInfo_SetValueForKey:
    //     Option<unsafe extern "C" fn(key: *const c_char, value: *const c_char)>,
    // pub GetPlayerUniqueID:
    //     Option<unsafe extern "C" fn(iPlayer: c_int, playerID: *mut [c_char; 16usize]) -> qboolean>,
    // pub GetTrackerIDForPlayer: Option<unsafe extern "C" fn(playerSlot: c_int) -> c_int>,
    // pub GetPlayerForTrackerID: Option<unsafe extern "C" fn(trackerID: c_int) -> c_int>,
    // pub pfnServerCmdUnreliable: Option<unsafe extern "C" fn(szCmdString: *mut c_char) -> c_int>,
    // pub pfnGetMousePos: Option<unsafe extern "C" fn(ppt: *mut tagPOINT)>,

    pub fn set_mouse_position(&self, x: c_int, y: c_int) {
        unsafe { unwrap!(self, pfnSetMousePos)(x, y) }
    }

    // pub pfnSetMouseEnable: Option<unsafe extern "C" fn(fEnable: qboolean)>,
    // pub pfnGetFirstCvarPtr: Option<unsafe extern "C" fn() -> *mut cvar_s>,
    // pub pfnGetFirstCmdFunctionHandle: Option<unsafe extern "C" fn() -> *mut c_void>,
    // pub pfnGetNextCmdFunctionHandle:
    //     Option<unsafe extern "C" fn(cmdhandle: *mut c_void) -> *mut c_void>,
    // pub pfnGetCmdFunctionName:
    //     Option<unsafe extern "C" fn(cmdhandle: *mut c_void) -> *const c_char>,
    // pub pfnGetClientOldTime: Option<unsafe extern "C" fn() -> f32>,
    // pub pfnGetGravity: Option<unsafe extern "C" fn() -> f32>,
    // pub pfnGetModelByIndex: Option<unsafe extern "C" fn(index: c_int) -> *mut model_s>,
    // pub pfnSetFilterMode: Option<unsafe extern "C" fn(mode: c_int)>,
    // pub pfnSetFilterColor: Option<unsafe extern "C" fn(red: f32, green: f32, blue: f32)>,
    // pub pfnSetFilterBrightness: Option<unsafe extern "C" fn(brightness: f32)>,
    // pub pfnSequenceGet: Option<
    //     unsafe extern "C" fn(fileName: *const c_char, entryName: *const c_char) -> *mut c_void,
    // >,
    // pub pfnSPR_DrawGeneric: Option<
    //     unsafe extern "C" fn(
    //         frame: c_int,
    //         x: c_int,
    //         y: c_int,
    //         prc: *const wrect_t,
    //         blendsrc: c_int,
    //         blenddst: c_int,
    //         width: c_int,
    //         height: c_int,
    //     ),
    // >,
    // pub pfnSequencePickSentence: Option<
    //     unsafe extern "C" fn(
    //         groupName: *const c_char,
    //         pickMethod: c_int,
    //         entryPicked: *mut c_int,
    //     ) -> *mut c_void,
    // >,

    pub fn draw_string(&self, x: c_int, y: c_int, s: impl ToEngineStr, color: RGB) -> c_int {
        let s = s.to_engine_str();
        let [r, g, b] = color.into();
        unsafe { unwrap!(self, pfnDrawString)(x, y, s.as_ptr(), r, g, b) }
    }

    pub fn draw_string_reverse(
        &self,
        x: c_int,
        y: c_int,
        s: impl ToEngineStr,
        color: RGB,
    ) -> c_int {
        let s = s.to_engine_str();
        let [r, g, b] = color.into();
        unsafe { unwrap!(self, pfnDrawStringReverse)(x, y, s.as_ptr(), r, g, b) }
    }

    // pub LocalPlayerInfo_ValueForKey:
    //     Option<unsafe extern "C" fn(key: *const c_char) -> *const c_char>,
    // pub pfnVGUI2DrawCharacter:
    //     Option<unsafe extern "C" fn(x: c_int, y: c_int, ch: c_int, font: c_uint) -> c_int>,
    // pub pfnVGUI2DrawCharacterAdditive: Option<
    //     unsafe extern "C" fn(
    //         x: c_int,
    //         y: c_int,
    //         ch: c_int,
    //         r: c_int,
    //         g: c_int,
    //         b: c_int,
    //         font: c_uint,
    //     ) -> c_int,
    // >,
    // pub pfnGetApproxWavePlayLen: Option<unsafe extern "C" fn(filename: *const c_char) -> c_uint>,
    // pub GetCareerGameUI: Option<unsafe extern "C" fn() -> *mut c_void>,
    // pub pfnIsPlayingCareerMatch: Option<unsafe extern "C" fn() -> c_int>,
    // pub pfnPlaySoundVoiceByName:
    //     Option<unsafe extern "C" fn(szSound: *mut c_char, volume: f32, pitch: c_int)>,
    // pub pfnPrimeMusicStream: Option<unsafe extern "C" fn(filename: *mut c_char, looping: c_int)>,
    // pub pfnProcessTutorMessageDecayBuffer:
    //     Option<unsafe extern "C" fn(buffer: *mut c_int, buflen: c_int)>,
    // pub pfnConstructTutorMessageDecayBuffer:
    //     Option<unsafe extern "C" fn(buffer: *mut c_int, buflen: c_int)>,
    // pub pfnResetTutorMessageDecayData: Option<unsafe extern "C" fn()>,
    // pub pfnPlaySoundByNameAtPitch:
    //     Option<unsafe extern "C" fn(szSound: *mut c_char, volume: f32, pitch: c_int)>,

    pub fn fill_rgba_blend(&self, x: c_int, y: c_int, width: c_int, height: c_int, color: RGBA) {
        unsafe {
            let [r, g, b, a] = color.into();
            unwrap!(self, pfnFillRGBABlend)(x, y, width, height, r, g, b, a)
        }
    }

    // pub pfnGetAppID: Option<unsafe extern "C" fn() -> c_int>,
    // pub pfnGetAliases: Option<unsafe extern "C" fn() -> *mut cmdalias_t>,
    // pub pfnVguiWrap2_GetMouseDelta: Option<unsafe extern "C" fn(x: *mut c_int, y: *mut c_int)>,
    // pub pfnFilteredClientCmd: Option<unsafe extern "C" fn(cmd: *const c_char) -> c_int>,

    pub fn is_singleplayer(&self) -> bool {
        self.get_max_clients() == 1
    }

    pub fn is_multiplayer(&self) -> bool {
        self.get_max_clients() > 1
    }
}

impl EngineCvar for ClientEngine {
    fn fn_get_cvar_float(&self) -> unsafe extern "C" fn(name: *const c_char) -> f32 {
        unwrap!(self, pfnGetCvarFloat)
    }

    fn fn_set_cvar_float(&self) -> unsafe extern "C" fn(name: *const c_char, value: f32) {
        unwrap!(self, Cvar_SetValue)
    }

    fn fn_get_cvar_string(&self) -> unsafe extern "C" fn(name: *const c_char) -> *const c_char {
        unwrap!(self, pfnGetCvarString)
    }

    fn fn_set_cvar_string(
        &self,
    ) -> unsafe extern "C" fn(name: *const c_char, value: *const c_char) {
        unwrap!(self, Cvar_Set)
    }
}

impl EngineRng for ClientEngine {
    fn fn_random_float(&self) -> unsafe extern "C" fn(min: f32, max: f32) -> f32 {
        unwrap!(self, pfnRandomFloat)
    }

    fn fn_random_int(&self) -> unsafe extern "C" fn(min: c_int, max: c_int) -> c_int {
        unwrap!(self, pfnRandomLong)
    }
}

impl EngineConsole for ClientEngine {
    fn console_print(&self, msg: impl ToEngineStr) {
        let msg = msg.to_engine_str();
        unsafe { unwrap!(self, pfnConsolePrint)(msg.as_ptr()) }
    }
}

impl EngineCmd for ClientEngine {
    fn fn_cmd_argc(&self) -> unsafe extern "C" fn() -> c_int {
        unwrap!(self, Cmd_Argc)
    }

    fn fn_cmd_argv(&self) -> unsafe extern "C" fn(argc: c_int) -> *const c_char {
        unwrap!(self, Cmd_Argv)
    }

    fn add_command(
        &self,
        name: impl ToEngineStr,
        func: unsafe extern "C" fn(),
    ) -> Result<(), AddCmdError> {
        let name = name.to_engine_str();
        let result = unsafe { unwrap!(self, pfnAddCommand)(name.as_ptr(), Some(func)) };
        if result == 0 {
            Err(AddCmdError)
        } else {
            Ok(())
        }
    }
}

impl EngineSystemTime for ClientEngine {
    fn system_time_f64(&self) -> f64 {
        unsafe { unwrap!(self, pfnSys_FloatTime)() }
    }
}

impl EngineDrawConsoleString for ClientEngine {
    fn set_text_color(&self, color: impl Into<RGB>) {
        let color = color.into();
        self.set_text_color_f32(
            color.r() as f32 * (1.0 / 255.0),
            color.g() as f32 * (1.0 / 255.0),
            color.b() as f32 * (1.0 / 255.0),
        );
    }

    fn console_string_size(&self, text: impl ToEngineStr) -> (c_int, c_int) {
        let text = text.to_engine_str();
        let mut width = 0;
        let mut height = 0;
        unsafe {
            unwrap!(self, pfnDrawConsoleStringLen)(text.as_ptr(), &mut width, &mut height);
        }
        (width, height)
    }

    fn draw_console_string(&self, x: c_int, y: c_int, text: impl ToEngineStr) -> c_int {
        let text = text.to_engine_str();
        // FIXME: ffi: why string is mutable?
        unsafe { unwrap!(self, pfnDrawConsoleString)(x, y, text.as_ptr().cast_mut()) }
    }
}

impl EngineNet for ClientEngine {
    fn net_api(&self) -> &NetApi {
        &self.net_api
    }
}

impl EngineFile for ClientEngine {
    fn load_file_raw(&self, path: &CStrThin, len: &mut i32) -> *mut u8 {
        // NOTE: usehunk is not used in the engine
        unsafe { unwrap!(self, COM_LoadFile)(path.as_ptr(), 0, len) }
    }

    unsafe fn free_file_raw(&self, file: *mut u8) {
        unsafe { unwrap!(self, COM_FreeFile)(file.cast()) }
    }
}
