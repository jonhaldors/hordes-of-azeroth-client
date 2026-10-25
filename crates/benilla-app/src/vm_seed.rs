use benilla_ui::script::UiScript;
use bevy::prelude::*;

type Seed = Box<dyn Fn(&UiScript)>;

#[derive(Default)]
pub(crate) struct VmSeeds(Vec<Seed>);

pub fn seed_every_vm(app: &mut App, seed: impl Fn(&UiScript) + 'static) {
    if !app.world().contains_non_send::<VmSeeds>() {
        app.insert_non_send_resource(VmSeeds::default());
    }
    app.world_mut()
        .non_send_resource_mut::<VmSeeds>()
        .0
        .push(Box::new(seed));
}

pub(crate) fn seed(world: &World, script: &UiScript) {
    if let Some(seeds) = world.get_non_send_resource::<VmSeeds>() {
        for seed in &seeds.0 {
            seed(script);
        }
    }
}
