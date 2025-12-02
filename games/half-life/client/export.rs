use core::{
    cell::RefCell,
    ffi::{c_int, c_uint},
    ptr,
};

use xash3d_client::{
    csz::CStrThin,
    entity::{EntityType, TempEntityList},
    export::{ClientDll, UnsyncGlobal, export_dll, impl_unsync_global},
    ffi::{
        self,
        api::{
            efx::TEMPENTITY,
            studio::{engine_studio_api_s, r_studio_interface_s},
        },
        client::client_data_s,
        common::{
            cl_entity_s, clientdata_s, entity_state_s, kbutton_t, local_state_s, ref_params_s,
            usercmd_s, vec3_t, weapon_data_s,
        },
    },
    prelude::*,
};

use crate::{
    camera::Camera, entity::Entities, events::Events, hud::Hud, input::Input,
    studio::StudioRenderer, view::View, weapons::Weapons,
};

pub struct Dll {
    events: RefCell<Events>,
    entities: RefCell<Entities>,
    input: RefCell<Input>,
    camera: RefCell<Camera>,
    view: RefCell<View>,
    hud: RefCell<Hud>,
    weapons: RefCell<Weapons>,
    renderer: RefCell<StudioRenderer>,
}

impl_unsync_global!(Dll);

macro_rules! impl_global_getter {
    ($ty:ty, $name:ident $(, $name_mut:ident)? $(,)?) => {
        #[allow(dead_code)]
        pub fn $name<'a>() -> core::cell::Ref<'a, $ty> {
            unsafe { Dll::global_assume_init_ref() }.$name.borrow()
        }

        $(
            #[allow(dead_code)]
            pub fn $name_mut<'a>() -> core::cell::RefMut<'a, $ty> {
                unsafe { Dll::global_assume_init_ref() }.$name.borrow_mut()
            }
        )?
    };
}

impl_global_getter!(Events, events);
impl_global_getter!(Entities, entities);
impl_global_getter!(Input, input);
impl_global_getter!(Camera, camera);
impl_global_getter!(View, view);
impl_global_getter!(Hud, hud);
impl_global_getter!(Weapons, weapons);
impl_global_getter!(StudioRenderer, renderer);

impl ClientDll for Dll {
    fn new(engine: ClientEngineRef) -> Self {
        Self {
            events: Events::new(engine).into(),
            entities: Entities::new(engine).into(),
            input: Input::new(engine).into(),
            camera: Camera::new(engine).into(),
            view: View::new(engine).into(),
            hud: Hud::new(engine).into(),
            weapons: Weapons::new(engine).into(),
            renderer: StudioRenderer::new(engine).into(),
        }
    }

    fn vid_init(&self) -> bool {
        self.hud.borrow_mut().vid_init();
        true
    }

    fn redraw(&self, time: f32, intermission: bool) -> bool {
        self.hud.borrow_mut().draw(time, intermission)
    }

    fn update_client_data(&self, data: &mut client_data_s, time: f32) -> bool {
        self.input.borrow().in_commands();
        self.hud.borrow_mut().update_client_data(data, time)
    }

    fn reset(&self) {
        self.hud.borrow_mut().reset();
    }

    fn txfer_local_overrides(&self, state: &mut entity_state_s, client: &clientdata_s) {
        self.entities.borrow().txfer_local_overrides(state, client);
    }

    fn process_player_state(&self, dst: &mut entity_state_s, src: &entity_state_s) {
        self.entities.borrow_mut().process_player_state(dst, src);
    }

    fn txfer_prediction_data(
        &self,
        ps: &mut entity_state_s,
        pps: &entity_state_s,
        pcd: &mut clientdata_s,
        ppcd: &clientdata_s,
        wd: *mut weapon_data_s,
        pwd: *const weapon_data_s,
    ) {
        let wd = unsafe { &mut *wd.cast() };
        let pwd = unsafe { &*pwd.cast() };
        self.entities
            .borrow_mut()
            .txfer_prediction_data(ps, pps, pcd, ppcd, wd, pwd);
    }

    fn create_move(&self, frame_time: f32, active: bool) -> usercmd_s {
        let hud = self.hud.borrow();
        let view = self.view.borrow();
        let mut input = self.input.borrow_mut();
        input.create_move(&hud, &view, frame_time, active)
    }

    fn post_run_cmd(
        &self,
        from: &mut local_state_s,
        to: &mut local_state_s,
        cmd: &mut usercmd_s,
        run_funcs: bool,
        time: f64,
        random_seed: c_uint,
    ) {
        self.weapons
            .borrow_mut()
            .post_run_cmd(from, to, cmd, run_funcs, time, random_seed);
    }

    fn calc_ref_def(&self, params: &mut ref_params_s) {
        self.view.borrow_mut().calc_ref_def(params);
    }

    fn is_third_person(&self) -> bool {
        self.camera.borrow().is_third_person()
    }

    fn camera_offset(&self) -> vec3_t {
        self.camera.borrow().offset()
    }

    fn camera_think(&self) {
        self.camera.borrow_mut().think();
    }

    fn kb_find(&self, name: &CStrThin) -> *mut kbutton_t {
        self.input
            .borrow()
            .keys
            .find(name)
            .unwrap_or(ptr::null_mut())
    }

    fn mouse_event(&self, mstate: c_int) {
        self.input.borrow_mut().mouse_event(mstate);
    }

    fn clear_states(&self) {
        self.input.borrow_mut().clear_states();
    }

    fn accumulate(&self) {
        self.input.borrow_mut().accumulate();
    }

    fn activate_mouse(&self) {
        self.input.borrow_mut().activate_mouse();
    }

    fn deactivate_mouse(&self) {
        self.input.borrow_mut().deactivate_mouse();
    }

    fn add_entity(&self, ty: EntityType, ent: &mut cl_entity_s, model_name: &CStrThin) -> bool {
        self.entities.borrow().add_entity(ty, ent, model_name)
    }

    fn create_entities(&self) {
        self.entities.borrow().create_entities();
    }

    fn update_temp_entities(
        &self,
        frametime: f64,
        client_time: f64,
        cl_gravity: f64,
        list: &mut TempEntityList,
        add_visible_entity: impl FnMut(&mut cl_entity_s) -> c_int,
        play_sound: impl FnMut(&mut TEMPENTITY, f32),
    ) {
        self.entities.borrow().update_temp_entities(
            frametime,
            client_time,
            cl_gravity,
            list,
            add_visible_entity,
            play_sound,
        );
    }

    fn get_studio_model_interface(
        &self,
        version: c_int,
        interface: *mut *mut r_studio_interface_s,
        studio: *mut engine_studio_api_s,
    ) -> bool {
        // TODO: export studio interface
        if true {
            return false;
        }

        if version != ffi::api::studio::STUDIO_INTERFACE_VERSION {
            return false;
        }

        unsafe {
            ptr::write(interface, ptr::addr_of_mut!(STUDIO));
        }

        unsafe {
            xash3d_client::instance::init_studio(&*studio);
        }

        true
    }
}

export_dll!(Dll);

#[allow(non_snake_case)]
unsafe extern "C" fn StudioDrawModel(flags: c_int) -> c_int {
    renderer().draw_model(flags)
}

#[allow(non_snake_case)]
unsafe extern "C" fn StudioDrawPlayer(flags: c_int, player: *mut entity_state_s) -> c_int {
    let player = unsafe { &mut *player };
    renderer().draw_player(flags, player)
}

static mut STUDIO: r_studio_interface_s = r_studio_interface_s {
    version: xash3d_client::ffi::api::studio::STUDIO_INTERFACE_VERSION,
    StudioDrawModel: Some(StudioDrawModel),
    StudioDrawPlayer: Some(StudioDrawPlayer),
};
