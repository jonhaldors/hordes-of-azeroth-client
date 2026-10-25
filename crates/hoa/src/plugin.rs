use bevy::prelude::*;

use crate::interact::{update_interact, SharedInteract};
use crate::lua_api::{scripted_login_from_env, LuaHosts};
use crate::nameplates::{add_plate_events, track_nameplates, SharedPlates};
use crate::snapshot::{capture_snapshot, SharedSnapshot};
use crate::world_map::SharedMapAreas;

pub struct HordesOfAzerothPlugin;

impl Plugin for HordesOfAzerothPlugin {
    fn build(&self, app: &mut App) {
        add_plate_events();
        let hosts = LuaHosts {
            snapshot: SharedSnapshot::default(),
            areas: SharedMapAreas::from_install(),
            interact: SharedInteract::default(),
            plates: SharedPlates::default(),
            scripted_login: scripted_login_from_env(),
        };
        app.insert_non_send_resource(hosts.snapshot.clone())
            .insert_non_send_resource(hosts.interact.clone())
            .insert_non_send_resource(hosts.plates.clone())
            .add_systems(Startup, (name_window, announce_addon_folder))
            .add_systems(
                Last,
                (capture_snapshot, update_interact, track_nameplates).chain(),
            );
        benilla_app::seed_every_vm(app, move |script| hosts.seed(script));
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
