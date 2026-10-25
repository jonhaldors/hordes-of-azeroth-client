use bevy::prelude::*;

use super::spell_visual::{FxClass, FxSlot, FxStage, SpellKitFx};
use super::{KitPush, PlaySeq};
use crate::entities::VisualAttached;

#[derive(Component, Default)]
#[require(Transform, Visibility, VisualAttached)]
pub struct FxAnchor;

#[derive(Message, Clone, Debug)]
pub enum AnchorFx {
    Kit { anchor: Entity, kit_id: u32 },
    Model { anchor: Entity, path: String },
}

impl AnchorFx {
    fn anchor(&self) -> Entity {
        match self {
            AnchorFx::Kit { anchor, .. } | AnchorFx::Model { anchor, .. } => *anchor,
        }
    }
}

const ANCHOR_MODEL_EFFECT: u32 = u32::MAX;
const FRAMES_TO_WAIT_FOR_ANCHOR: u8 = 3;

pub(crate) fn play_anchor_fx(
    mut requests: MessageReader<AnchorFx>,
    mut waiting: Local<Vec<(AnchorFx, u8)>>,
    anchors: Query<(), With<FxAnchor>>,
    mut play_seq: ResMut<PlaySeq>,
    mut kits: MessageWriter<KitPush>,
    mut models: MessageWriter<SpellKitFx>,
) {
    let mut pending = std::mem::take(&mut *waiting);
    pending.extend(requests.read().cloned().map(|r| (r, 0)));
    for (request, frames) in pending {
        let anchor = request.anchor();
        if anchors.get(anchor).is_err() {
            if frames < FRAMES_TO_WAIT_FOR_ANCHOR {
                waiting.push((request, frames + 1));
            }
            continue;
        }
        match request {
            AnchorFx::Kit { kit_id, .. } => {
                kits.write(KitPush {
                    entity: anchor,
                    kit_id,
                    seq: play_seq.next(),
                });
            }
            AnchorFx::Model { path, .. } => {
                models.write(SpellKitFx::Begin {
                    entity: anchor,
                    spell_id: 0,
                    persistent: true,
                    class: FxClass::Hold,
                    stage: FxStage::Relive,
                    effects: vec![FxSlot {
                        tag: 0,
                        effect: ANCHOR_MODEL_EFFECT,
                        path,
                    }],
                });
            }
        }
    }
}
