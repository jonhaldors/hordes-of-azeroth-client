use bevy::prelude::*;

use crate::controller_lua::deliver_pad_events;
use crate::interact::{update_interact, SharedInteract};
use crate::keys::{capture_typing, SharedTyping};
use crate::lua_api::{scripted_login_from_env, LuaHosts};
use crate::nameplates::{add_plate_events, track_nameplates, SharedPlates};
use crate::pad::{poll_pads, SharedController};
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
            typing: SharedTyping::default(),
            controller: SharedController::default(),
            scripted_login: scripted_login_from_env(),
        };
        app.add_plugins(bevy::gilrs::GilrsPlugin)
            .insert_non_send_resource(hosts.snapshot.clone())
            .insert_non_send_resource(hosts.interact.clone())
            .insert_non_send_resource(hosts.plates.clone())
            .insert_non_send_resource(hosts.typing.clone())
            .insert_non_send_resource(hosts.controller.clone())
            .add_systems(Startup, (name_window, announce_addon_folder))
            .add_systems(PreUpdate, poll_pads)
            .add_systems(
                Last,
                (
                    capture_snapshot,
                    update_interact,
                    track_nameplates,
                    capture_typing,
                    deliver_pad_events,
                )
                    .chain(),
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
