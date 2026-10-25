use bevy::prelude::*;

use crate::interact::{update_interact, SharedInteract};
use crate::lua_api::{install_lua_api, InstalledSession};
use crate::snapshot::{capture_snapshot, SharedSnapshot};
use crate::world_map::SharedMapAreas;

pub struct HordesOfAzerothPlugin;

impl Plugin for HordesOfAzerothPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(SharedSnapshot::default())
            .insert_non_send_resource(SharedMapAreas::from_install())
            .insert_non_send_resource(SharedInteract::default())
            .init_resource::<InstalledSession>()
            .add_systems(Startup, (name_window, announce_addon_folder))
            .add_systems(
                Last,
                (capture_snapshot, update_interact, install_lua_api).chain(),
            );
    }
}

const WINDOW_TITLE: &str = "Hordes of Azeroth";

fn name_window(mut windows: Query<&mut Window>) {
    for mut window in &mut windows {
        window.title = WINDOW_TITLE.to_string();
    }
}

fn announce_addon_folder() {
    let addons = benilla_app::config_dir().map(|dir| dir.join("AddOns"));
    info!("Hordes of Azeroth plugin running; addons load from {addons:?}");
}
