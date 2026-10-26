use std::collections::HashMap;

use benilla_app::Objects;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use benilla_world::decal::{DecalFrame, WorldDecal};
use benilla_world::particles::buffer::{EffectVertex, WorldEffectDraw};
use benilla_world::sky_order::Rung;
use benilla_world::view::WorldCamera;

use crate::server_draw::{MarkSpec, Shape, TELEGRAPH_KEY_PREFIX};

const LINE_PIECE_YARDS: f32 = 3.0;
const LINE_HALF_WIDTH: f32 = 1.5;
const MAX_LINE_PIECES: u32 = 12;
const CONE_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_4;
const RIM_YARDS: f32 = 0.3;
const SLAB_YARDS: f32 = 4.0;
const PIXELS_PER_YARD: f32 = 12.0;
const MIN_TEXTURE_PIXELS: u32 = 64;
const MAX_TEXTURE_PIXELS: u32 = 512;
const SMALLEST_FILL: f32 = 0.02;
const TEXTURE_SIZE_STEPS_PER_YARD: f32 = 2.0;
const FOLLOW_MOVE_YARDS: f32 = 0.05;

#[derive(Clone, Copy)]
struct Palette {
    rim: [f32; 3],
    wash: [f32; 3],
}

const PALETTES: [Palette; 5] = [
    Palette {
        rim: [1.0, 0.30, 0.18],
        wash: [0.85, 0.04, 0.02],
    },
    Palette {
        rim: [0.75, 0.55, 1.0],
        wash: [0.35, 0.20, 0.90],
    },
    Palette {
        rim: [0.55, 1.0, 0.45],
        wash: [0.15, 0.75, 0.10],
    },
    Palette {
        rim: [0.65, 0.90, 1.0],
        wash: [0.20, 0.55, 0.95],
    },
    Palette {
        rim: [0.85, 0.10, 0.10],
        wash: [0.45, 0.02, 0.02],
    },
];

const OUTLINE_RIM_ALPHA: f32 = 0.95;
const OUTLINE_WASH_ALPHA: f32 = 0.12;
const FILL_WASH_ALPHA: f32 = 0.38;
const FILL_FRONT_ALPHA: f32 = 0.7;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Layer {
    Outline,
    Fill,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    shape: Shape,
    size_steps: u32,
    style: u8,
    layer: Layer,
}

struct Mark {
    spec: MarkSpec,
    started: f32,
    surfaces: usize,
    verts: Vec<EffectVertex>,
}

#[derive(Resource, Default)]
pub struct Telegraphs {
    live: HashMap<String, Mark>,
    baked: HashMap<TextureKey, Handle<Image>>,
    loaded: HashMap<String, Handle<Image>>,
}

impl Telegraphs {
    pub fn add(&mut self, spec: MarkSpec, now: f32) {
        self.live.insert(
            spec.key.clone(),
            Mark {
                started: now - spec.done,
                spec,
                surfaces: 0,
                verts: Vec::new(),
            },
        );
    }

    pub fn remove(&mut self, key: &str) {
        self.live.remove(key);
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn clear_all(&mut self) {
        self.live.clear();
    }

    pub fn clear_telegraphs(&mut self) {
        self.live.retain(|key, _| !key.starts_with(TELEGRAPH_KEY_PREFIX));
    }
}

fn line_length(size: f32) -> f32 {
    let pieces = (size / LINE_PIECE_YARDS).round().clamp(1.0, MAX_LINE_PIECES as f32);
    pieces * LINE_PIECE_YARDS
}

fn drawn_size(spec: &MarkSpec) -> f32 {
    match spec.shape {
        Shape::Line => line_length(spec.size),
        Shape::Circle | Shape::Cone => spec.size.max(0.5),
    }
}

fn extents(shape: Shape, size: f32) -> (f32, f32, f32, f32) {
    match shape {
        Shape::Circle | Shape::Cone => (-size, size, -size, size),
        Shape::Line => (-LINE_HALF_WIDTH, LINE_HALF_WIDTH, -size, 0.0),
    }
}

fn frame_for(spec: &MarkSpec) -> DecalFrame {
    let (min_x, max_x, min_z, max_z) = extents(spec.shape, drawn_size(spec));
    let (sin, cos) = spec.yaw.sin_cos();
    DecalFrame {
        center: spec.at,
        sin,
        cos,
        min_x,
        max_x,
        min_z,
        max_z,
        min_y: -SLAB_YARDS,
        max_y: SLAB_YARDS,
    }
}

fn fill_uv(shape: Shape, uv: [f32; 2], progress: f32) -> [f32; 2] {
    let p = progress.max(SMALLEST_FILL);
    match shape {
        Shape::Circle | Shape::Cone => [0.5 + (uv[0] - 0.5) / p, 0.5 + (uv[1] - 0.5) / p],
        Shape::Line => [uv[0], 1.0 - (1.0 - uv[1]) / p],
    }
}

fn band(distance_inside: f32) -> f32 {
    (1.0 - (distance_inside / RIM_YARDS - 1.0).abs()).clamp(0.0, 1.0)
}

fn distance_inside(shape: Shape, size: f32, x: f32, z: f32) -> f32 {
    match shape {
        Shape::Circle => size - (x * x + z * z).sqrt(),
        Shape::Cone => {
            let radius = (x * x + z * z).sqrt();
            let along = -z;
            let across = x.abs();
            let (sin, cos) = CONE_HALF_ANGLE.sin_cos();
            let to_edge = along * sin - across * cos;
            (size - radius).min(to_edge)
        }
        Shape::Line => (LINE_HALF_WIDTH - x.abs()).min(-z).min(size + z),
    }
}

fn texel(key: TextureKey, size: f32, x: f32, z: f32) -> [u8; 4] {
    let palette = PALETTES[usize::from(key.style).min(PALETTES.len() - 1)];
    let inside = distance_inside(key.shape, size, x, z);
    let (colour, alpha) = if inside <= 0.0 {
        (palette.wash, 0.0)
    } else {
        match key.layer {
            Layer::Outline => {
                let rim = band(inside);
                let colour = std::array::from_fn(|i| palette.wash[i] + (palette.rim[i] - palette.wash[i]) * rim);
                (colour, OUTLINE_WASH_ALPHA + (OUTLINE_RIM_ALPHA - OUTLINE_WASH_ALPHA) * rim)
            }
            Layer::Fill => {
                let front = band(inside);
                (palette.wash, FILL_WASH_ALPHA + (FILL_FRONT_ALPHA - FILL_WASH_ALPHA) * front)
            }
        }
    };
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(colour[0]), byte(colour[1]), byte(colour[2]), byte(alpha)]
}

fn texture_pixels(yards: f32) -> u32 {
    ((yards * PIXELS_PER_YARD) as u32).clamp(MIN_TEXTURE_PIXELS, MAX_TEXTURE_PIXELS)
}

fn bake(key: TextureKey) -> Image {
    let size = key.size_steps as f32 / TEXTURE_SIZE_STEPS_PER_YARD;
    let (min_x, max_x, min_z, max_z) = extents(key.shape, size);
    let (width, height) = (texture_pixels(max_x - min_x), texture_pixels(max_z - min_z));
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let v = (row as f32 + 0.5) / height as f32;
        let z = min_z + v * (max_z - min_z);
        for column in 0..width {
            let u = (column as f32 + 0.5) / width as f32;
            let x = min_x + u * (max_x - min_x);
            let edge = row == 0 || column == 0 || row + 1 == height || column + 1 == width;
            let pixel = if edge { [0; 4] } else { texel(key, size, x, z) };
            data.extend_from_slice(&pixel);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

fn mpq_path(texture: &str) -> String {
    format!("mpq://{}", texture.replace('\\', "/").to_lowercase())
}

impl Telegraphs {
    fn baked_texture(&mut self, images: &mut Assets<Image>, spec: &MarkSpec, layer: Layer) -> AssetId<Image> {
        let key = TextureKey {
            shape: spec.shape,
            size_steps: (drawn_size(spec) * TEXTURE_SIZE_STEPS_PER_YARD).round().max(1.0) as u32,
            style: spec.style,
            layer,
        };
        self.baked
            .entry(key)
            .or_insert_with(|| images.add(bake(key)))
            .id()
    }

    fn game_texture(&mut self, assets: &AssetServer, texture: &str) -> AssetId<Image> {
        self.loaded
            .entry(texture.to_string())
            .or_insert_with(|| assets.load::<Image>(mpq_path(texture)))
            .id()
    }
}

pub fn expire_telegraphs(mut telegraphs: ResMut<Telegraphs>, time: Res<Time>) {
    let now = time.elapsed_secs();
    telegraphs
        .live
        .retain(|_, m| m.spec.duration.is_none_or(|d| now - m.started < d));
}

pub fn follow_marks(mut telegraphs: ResMut<Telegraphs>, objects: Objects, transforms: Query<&GlobalTransform>) {
    for mark in telegraphs.live.values_mut() {
        if mark.spec.follow == 0 {
            continue;
        }
        let Some(at) = objects
            .entity(mark.spec.follow)
            .and_then(|e| transforms.get(e).ok())
            .map(GlobalTransform::translation)
        else {
            continue;
        };
        if at.distance(mark.spec.at) > FOLLOW_MOVE_YARDS {
            mark.spec.at = at;
            mark.verts.clear();
        }
    }
}

pub fn project_telegraphs(mut telegraphs: ResMut<Telegraphs>, decals: WorldDecal) {
    let surfaces = decals.receiver_count();
    for mark in telegraphs.live.values_mut() {
        if mark.surfaces == surfaces && !mark.verts.is_empty() {
            continue;
        }
        mark.surfaces = surfaces;
        mark.verts.clear();
        let frame = frame_for(&mark.spec);
        decals.project(
            &mut mark.verts,
            &frame,
            |p| ((SLAB_YARDS - p.y.abs()) / SLAB_YARDS).clamp(0.0, 1.0),
            |x, z| frame.rect_uv(x, z),
        );
    }
}

pub fn push_telegraphs(
    mut telegraphs: ResMut<Telegraphs>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    time: Res<Time>,
    cam: Query<Entity, With<WorldCamera>>,
    mut draw: WorldEffectDraw,
) {
    let Ok(cam) = cam.single() else { return };
    let now = time.elapsed_secs();
    let keys: Vec<String> = telegraphs.live.keys().cloned().collect();
    for key in keys {
        let Some(spec) = telegraphs.live.get(&key).map(|m| m.spec.clone()) else {
            continue;
        };
        let (outline, fill) = match &spec.texture {
            Some(texture) => (telegraphs.game_texture(&assets, texture), None),
            None => (
                telegraphs.baked_texture(&mut images, &spec, Layer::Outline),
                (!spec.outline_only).then(|| telegraphs.baked_texture(&mut images, &spec, Layer::Fill)),
            ),
        };
        let mark = &telegraphs.live[&key];
        if mark.verts.is_empty() {
            continue;
        }
        let white = |v: &EffectVertex, uv: [f32; 2]| EffectVertex {
            pos: v.pos,
            uv,
            color: [1.0, 1.0, 1.0, v.color[3]],
        };
        let batch = draw
            .batch(cam, outline)
            .anchored(spec.at)
            .rung(Rung::RETICLE, Rung::DECAL_RASTER);
        let mut batch = if spec.additive { batch.additive() } else { batch };
        batch.extend(mark.verts.iter().map(|v| white(v, v.uv)));
        batch.tris();
        let Some(fill) = fill else { continue };
        let progress = spec
            .duration
            .map_or(1.0, |d| ((now - mark.started) / d.max(0.05)).clamp(0.0, 1.0));
        let mut batch = draw
            .batch(cam, fill)
            .anchored(spec.at)
            .rung(Rung::RETICLE, Rung::DECAL_RASTER);
        batch.extend(mark.verts.iter().map(|v| white(v, fill_uv(spec.shape, v.uv, progress))));
        batch.tris();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_whole_three_yard_pieces_up_to_twelve() {
        assert_eq!(line_length(1.0), 3.0);
        assert_eq!(line_length(10.0), 9.0);
        assert_eq!(line_length(80.0), 36.0);
    }

    #[test]
    fn a_half_filled_circle_samples_the_fill_disc_twice_as_wide() {
        assert_eq!(fill_uv(Shape::Circle, [0.75, 0.5], 0.5), [1.0, 0.5]);
        assert_eq!(fill_uv(Shape::Circle, [0.5, 0.5], 0.5), [0.5, 0.5]);
    }

    #[test]
    fn a_line_fills_out_from_its_origin() {
        assert_eq!(fill_uv(Shape::Line, [0.5, 1.0], 0.25), [0.5, 1.0]);
        assert_eq!(fill_uv(Shape::Line, [0.5, 0.75], 0.25), [0.5, 0.0]);
    }

    #[test]
    fn the_cone_opens_forward_ninety_degrees() {
        assert!(distance_inside(Shape::Cone, 10.0, 0.0, -5.0) > 0.0);
        assert!(distance_inside(Shape::Cone, 10.0, 4.0, -5.0) > 0.0);
        assert!(distance_inside(Shape::Cone, 10.0, 6.0, -5.0) < 0.0);
        assert!(distance_inside(Shape::Cone, 10.0, 0.0, 5.0) < 0.0);
    }

    #[test]
    fn the_frame_points_forward_along_the_facing() {
        use benilla_assets::coords::wow_to_bevy;
        let facing = 0.7_f32;
        let forward = wow_to_bevy([facing.cos(), facing.sin(), 0.0]);
        let (sin, cos) = facing.sin_cos();
        let z_in_frame = forward.z * cos + forward.x * sin;
        let x_in_frame = forward.x * cos - forward.z * sin;
        assert!((z_in_frame + 1.0).abs() < 1e-5);
        assert!(x_in_frame.abs() < 1e-5);
    }

    #[test]
    fn a_game_texture_loads_from_the_archive_path() {
        assert_eq!(
            mpq_path("World\\Goober\\RuneGroundBlue01.blp"),
            "mpq://world/goober/runegroundblue01.blp"
        );
    }
}
