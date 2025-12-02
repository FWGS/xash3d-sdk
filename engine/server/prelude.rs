pub use xash3d_shared::prelude::*;

pub use crate::{
    engine::prelude::*,
    engine::{ServerEngine, ServerEngineRef},
};

pub use crate::{
    entity::{AsEntityHandle, CreateEntity, Entity},
    global_state::{GlobalStateRef, decals::Decals},
    private::{GetPrivateData, PrivateEntity},
};

#[cfg(feature = "save")]
pub use crate::save::{Restore, Save};
