use std::cell::RefCell;
use std::f32::consts::TAU;
use std::rc::Rc;

use benilla_app::Objects;
use benilla_assets::coords::bevy_to_wow;
use benilla_protocol::guid;
use benilla_world::world_map::CurrentMap;
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WowPosition {
    pub north: f32,
    pub west: f32,
    pub up: f32,
}

impl WowPosition {
    pub fn from_bevy(at: Vec3) -> Self {
        let [north, west, up] = bevy_to_wow(at);
        Self { north, west, up }
    }

    pub fn distance_to(self, other: WowPosition) -> f32 {
        let dn = self.north - other.north;
        let dw = self.west - other.west;
        let du = self.up - other.up;
        (dn * dn + dw * dw + du * du).sqrt()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerView {
    pub position: WowPosition,
    pub facing: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CreatureView {
    pub entry: u32,
    pub position: WowPosition,
}

#[derive(Default, Debug)]
pub struct Snapshot {
    pub player: Option<PlayerView>,
    pub map_id: u32,
    pub creatures: Vec<CreatureView>,
}

impl Snapshot {
    pub fn closest_creature(&self, entry: u32) -> Option<(WowPosition, f32)> {
        let from = self.player?.position;
        self.creatures
            .iter()
            .filter(|creature| creature.entry == entry)
            .map(|creature| (creature.position, from.distance_to(creature.position)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }
}

#[derive(Clone, Default)]
pub struct SharedSnapshot(pub Rc<RefCell<Snapshot>>);

pub fn facing_from_rotation(rotation: Quat) -> f32 {
    rotation.to_euler(EulerRot::YXZ).0.rem_euclid(TAU)
}

fn creature_entry(unit_guid: u64) -> Option<u32> {
    (guid::high(unit_guid) == guid::HIGH_UNIT)
        .then(|| guid::entry(unit_guid))
        .flatten()
}

pub fn capture_snapshot(
    objects: Objects,
    bodies: Query<&GlobalTransform>,
    map: Option<Res<CurrentMap>>,
    shared: NonSend<SharedSnapshot>,
) {
    let mut snapshot = shared.0.borrow_mut();
    snapshot.map_id = map.map_or(0, |map| map.0);
    snapshot.player = objects
        .player()
        .and_then(|me| bodies.get(me).ok())
        .map(|body| {
            let pose = body.compute_transform();
            PlayerView {
                position: WowPosition::from_bevy(pose.translation),
                facing: facing_from_rotation(pose.rotation),
            }
        });
    snapshot.creatures.clear();
    for (unit_guid, entity, _) in objects.iter() {
        let Some(entry) = creature_entry(unit_guid) else {
            continue;
        };
        if let Ok(body) = bodies.get(entity) {
            snapshot.creatures.push(CreatureView {
                entry,
                position: WowPosition::from_bevy(body.translation()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use benilla_assets::coords::wow_to_bevy;

    #[test]
    fn position_round_trips_through_bevy_space() {
        let at = WowPosition::from_bevy(wow_to_bevy([-8949.0, -132.5, 83.5]));
        assert_eq!(
            at,
            WowPosition {
                north: -8949.0,
                west: -132.5,
                up: 83.5
            }
        );
    }

    #[test]
    fn facing_is_the_yaw_kept_in_one_turn() {
        let facing = facing_from_rotation(Quat::from_rotation_y(-1.0));
        assert!((facing - (TAU - 1.0)).abs() < 1e-5);
    }

    #[test]
    fn closest_creature_picks_the_nearest_of_that_entry() {
        let at = |north| WowPosition {
            north,
            west: 0.0,
            up: 0.0,
        };
        let snapshot = Snapshot {
            player: Some(PlayerView {
                position: at(0.0),
                facing: 0.0,
            }),
            map_id: 0,
            creatures: vec![
                CreatureView {
                    entry: 7,
                    position: at(30.0),
                },
                CreatureView {
                    entry: 9,
                    position: at(1.0),
                },
                CreatureView {
                    entry: 7,
                    position: at(-10.0),
                },
            ],
        };
        assert_eq!(snapshot.closest_creature(7), Some((at(-10.0), 10.0)));
        assert_eq!(snapshot.closest_creature(8), None);
    }
}
