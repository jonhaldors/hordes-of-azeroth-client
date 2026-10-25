use benilla_mlua::{Lua, MultiValue, Result as LuaResult, Table};
use benilla_ui::script::UiScript;
use bevy::prelude::*;

use crate::interact::SharedInteract;
use crate::interact_lua::register_interact;
use crate::snapshot::SharedSnapshot;
use crate::world_map::SharedMapAreas;

#[derive(Resource, Default)]
pub struct InstalledSession(Option<u64>);

pub fn install_lua_api(world: &mut World) {
    let (Some(snapshot), Some(areas), Some(interact)) = (
        world.get_non_send_resource::<SharedSnapshot>().cloned(),
        world.get_non_send_resource::<SharedMapAreas>().cloned(),
        world.get_non_send_resource::<SharedInteract>().cloned(),
    ) else {
        return;
    };
    let Some(script) = world.get_non_send_resource::<UiScript>() else {
        return;
    };
    let session = script.session();
    if world.resource::<InstalledSession>().0 == Some(session) {
        return;
    }
    let installed = register_classic_api(script.lua(), &snapshot, &areas)
        .and_then(|()| register_interact(script.lua(), &interact));
    if let Err(error) = installed {
        warn!("Hordes of Azeroth: Lua functions not installed: {error}");
    }
    world.resource_mut::<InstalledSession>().0 = Some(session);
}

fn register_classic_api(
    lua: &Lua,
    snapshot: &SharedSnapshot,
    areas: &SharedMapAreas,
) -> LuaResult<()> {
    let globals = lua.globals();
    globals.set("UnitPosition", unit_position(lua, snapshot.clone())?)?;
    globals.set("GetPlayerFacing", player_facing(lua, snapshot.clone())?)?;
    globals.set(
        "ClosestUnitPosition",
        closest_unit_position(lua, snapshot.clone())?,
    )?;
    let c_map = match globals.get::<Option<Table>>("C_Map")? {
        Some(table) => table,
        None => {
            let table = lua.create_table()?;
            globals.set("C_Map", table.clone())?;
            table
        }
    };
    c_map.set("GetMapAreaIDs", map_area_ids(lua, areas.clone())?)?;
    c_map.set(
        "GetMapPosFromWorldPos",
        map_pos_from_world_pos(lua, areas.clone())?,
    )?;
    Ok(())
}

fn is_player_token(token: &str) -> bool {
    token.eq_ignore_ascii_case("player")
}

fn unit_position(lua: &Lua, snapshot: SharedSnapshot) -> LuaResult<benilla_mlua::Function> {
    lua.create_function(move |lua, token: String| {
        let view = snapshot.0.borrow();
        match view.player.filter(|_| is_player_token(&token)) {
            Some(player) => lua.pack_multi((
                f64::from(player.position.west),
                f64::from(player.position.north),
                f64::from(player.position.up),
                f64::from(view.map_id),
            )),
            None => lua.pack_multi(benilla_mlua::Nil),
        }
    })
}

fn player_facing(lua: &Lua, snapshot: SharedSnapshot) -> LuaResult<benilla_mlua::Function> {
    lua.create_function(move |lua, ()| match snapshot.0.borrow().player {
        Some(player) => lua.pack_multi(f64::from(player.facing)),
        None => Ok(MultiValue::new()),
    })
}

fn closest_unit_position(lua: &Lua, snapshot: SharedSnapshot) -> LuaResult<benilla_mlua::Function> {
    lua.create_function(move |lua, entry: f64| {
        match snapshot.0.borrow().closest_creature(entry as u32) {
            Some((position, distance)) => lua.pack_multi((
                f64::from(position.north),
                f64::from(position.west),
                f64::from(distance),
            )),
            None => Ok(MultiValue::new()),
        }
    })
}

fn map_area_ids(lua: &Lua, areas: SharedMapAreas) -> LuaResult<benilla_mlua::Function> {
    lua.create_function(move |lua, ()| {
        let table = lua.create_table()?;
        for (name, area_id) in areas.0.zone_ids() {
            table.set(name, f64::from(area_id))?;
        }
        Ok(table)
    })
}

fn map_pos_from_world_pos(lua: &Lua, areas: SharedMapAreas) -> LuaResult<benilla_mlua::Function> {
    lua.create_function(
        move |lua, (map_id, world, zone): (f64, Table, Option<f64>)| {
            let north: f64 = world.get("x")?;
            let west: f64 = world.get("y")?;
            let found = match zone {
                Some(zone) => areas
                    .0
                    .position_in_zone(zone as u32, north, west)
                    .map(|(across, down)| (zone as u32, across, down)),
                None => areas.0.position_on_map(map_id as u32, north, west),
            };
            let Some((area_id, across, down)) = found else {
                return lua.pack_multi(benilla_mlua::Nil);
            };
            let position = lua.create_table()?;
            position.set("x", across)?;
            position.set("y", down)?;
            lua.pack_multi((f64::from(area_id), position))
        },
    )
}
