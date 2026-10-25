use std::collections::HashMap;

use benilla_app::{AnchorFx, ChatTap, FxAnchor, TappedLine};
use benilla_formats::{load_creature_catalog, wow_data, Chain};
use bevy::prelude::*;

use crate::server_draw::{parse_line, DrawCommand, FlightSpec, KitSpec, DRAW_PREFIX};
use crate::telegraphs::Telegraphs;

#[derive(Resource, Default)]
pub struct DisplayModels(HashMap<u32, (String, f32)>);

impl DisplayModels {
    pub fn from_install() -> Self {
        match load_display_models() {
            Ok(models) => models,
            Err(error) => {
                warn!("Hordes of Azeroth: creature displays not loaded: {error:#}");
                Self::default()
            }
        }
    }
}

fn load_display_models() -> anyhow::Result<DisplayModels> {
    let data = wow_data().ok_or_else(|| anyhow::anyhow!("no WoW Data folder found"))?;
    let mut chain = Chain::open(&data)?;
    let catalog = load_creature_catalog(&mut chain)?;
    let models = catalog
        .display_models()
        .filter_map(|(display, _)| {
            let model = catalog.model(display)?;
            Some((display, (model.model_path, model.scale)))
        })
        .collect();
    Ok(DisplayModels(models))
}

#[derive(Component)]
pub struct KitAnchor {
    until: f32,
}

#[derive(Component)]
pub struct Flight {
    id: u32,
    from: Vec3,
    to: Vec3,
    started: f32,
    duration: f32,
}

pub fn tap_draw_lines(mut tap: ResMut<ChatTap>) {
    if !tap.prefixes.iter().any(|p| p == DRAW_PREFIX) {
        tap.prefixes.push(DRAW_PREFIX.to_string());
    }
}

pub fn apply_draw_lines(
    mut commands: Commands,
    mut lines: MessageReader<TappedLine>,
    mut telegraphs: ResMut<Telegraphs>,
    mut fx: MessageWriter<AnchorFx>,
    displays: Res<DisplayModels>,
    flights: Query<(Entity, &Flight)>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for line in lines.read() {
        let Some(command) = parse_line(&line.0) else {
            continue;
        };
        match command {
            DrawCommand::Telegraph(spec) => telegraphs.add(spec, now),
            DrawCommand::RemoveTelegraph(id) => telegraphs.remove(id),
            DrawCommand::ClearTelegraphs => telegraphs.clear(),
            DrawCommand::Kit(kit) => play_kit(&mut commands, &mut fx, kit, now),
            DrawCommand::Fly(flight) => fly(&mut commands, &mut fx, &displays, flight, now),
            DrawCommand::Land(id) => {
                for (entity, flight) in &flights {
                    if flight.id == id {
                        commands.entity(entity).despawn();
                    }
                }
            }
        }
    }
}

fn play_kit(commands: &mut Commands, fx: &mut MessageWriter<AnchorFx>, kit: KitSpec, now: f32) {
    let transform = Transform::from_translation(kit.at).with_rotation(Quat::from_rotation_y(kit.yaw));
    let anchor = commands
        .spawn((
            FxAnchor,
            transform,
            GlobalTransform::from(transform),
            KitAnchor {
                until: now + kit.hold,
            },
        ))
        .id();
    fx.write(AnchorFx::Kit {
        anchor,
        kit_id: kit.kit_id,
    });
}

fn flight_transform(from: Vec3, to: Vec3, at: Vec3, scale: f32) -> Transform {
    let travel = to - from;
    let transform = Transform::from_translation(at).with_scale(Vec3::splat(scale));
    if travel.length_squared() > 1e-6 {
        transform.looking_to(travel, Vec3::Y)
    } else {
        transform
    }
}

fn fly(
    commands: &mut Commands,
    fx: &mut MessageWriter<AnchorFx>,
    displays: &DisplayModels,
    flight: FlightSpec,
    now: f32,
) {
    let Some((path, model_scale)) = displays.0.get(&flight.display_id).cloned() else {
        warn!("Hordes of Azeroth: no model for bolt display {}", flight.display_id);
        return;
    };
    let transform = flight_transform(flight.from, flight.to, flight.from, model_scale * flight.scale);
    let anchor = commands
        .spawn((
            FxAnchor,
            transform,
            GlobalTransform::from(transform),
            Flight {
                id: flight.id,
                from: flight.from,
                to: flight.to,
                started: now,
                duration: flight.duration.max(0.01),
            },
        ))
        .id();
    fx.write(AnchorFx::Model { anchor, path });
}

pub fn move_flights(
    mut commands: Commands,
    mut flights: Query<(Entity, &Flight, &mut Transform)>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for (entity, flight, mut transform) in &mut flights {
        let progress = (now - flight.started) / flight.duration;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = flight.from.lerp(flight.to, progress.max(0.0));
    }
}

pub fn expire_kit_anchors(mut commands: Commands, anchors: Query<(Entity, &KitAnchor)>, time: Res<Time>) {
    let now = time.elapsed_secs();
    for (entity, anchor) in &anchors {
        if now >= anchor.until {
            commands.entity(entity).despawn();
        }
    }
}
