use benilla_assets::coords::wow_to_bevy;
use bevy::prelude::Vec3;

pub const DRAW_PREFIX: &str = "HOA~";
pub const TELEGRAPH_KEY_PREFIX: &str = "t";
const MARK_KEY_PREFIX: &str = "m";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Circle,
    Cone,
    Line,
    Ring,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkSpec {
    pub key: String,
    pub shape: Shape,
    pub at: Vec3,
    pub yaw: f32,
    pub size: f32,
    pub duration: Option<f32>,
    pub outline_only: bool,
    pub additive: bool,
    pub ground_effect: bool,
    pub style: u8,
    pub follow: u64,
    pub texture: Option<String>,
    pub done: f32,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KitSpec {
    pub kit_id: u32,
    pub at: Vec3,
    pub yaw: f32,
    pub hold: f32,
    pub raised: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightSpec {
    pub id: u32,
    pub from: Vec3,
    pub to: Vec3,
    pub duration: f32,
    pub display_id: u32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileSpec {
    pub spell_id: u32,
    pub from: Vec3,
    pub to: Vec3,
    pub hold: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropSpec {
    pub key: u32,
    pub display_id: u32,
    pub at: Vec3,
    pub yaw: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuraSpec {
    pub key: u32,
    pub spell_id: u32,
    pub at: Vec3,
    pub scale: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DrawCommand {
    Mark(MarkSpec),
    RemoveMark(String),
    ClearTelegraphs,
    ClearAll,
    Kit(KitSpec),
    Fly(FlightSpec),
    Land(u32),
    Missile(MissileSpec),
    Prop(PropSpec),
    RemoveProp(u32),
    Aura(AuraSpec),
    RemoveAura(u32),
}

const OUTLINE_ONLY: u32 = 1;
const ADDITIVE: u32 = 2;
const GROUND_EFFECT: u32 = 4;
const KIT_RAISED: u32 = 1;

struct Fields<'a>(std::str::Split<'a, char>);

impl Fields<'_> {
    fn next_str(&mut self) -> Option<&str> {
        self.0.next()
    }

    fn u32(&mut self) -> Option<u32> {
        self.0.next()?.parse().ok()
    }

    fn u64(&mut self) -> Option<u64> {
        self.0.next()?.parse().ok()
    }

    fn f32(&mut self) -> Option<f32> {
        self.0.next()?.parse().ok()
    }

    fn secs(&mut self) -> Option<f32> {
        Some(self.u32()? as f32 / 1000.0)
    }

    fn point(&mut self) -> Option<Vec3> {
        let (x, y, z) = (self.f32()?, self.f32()?, self.f32()?);
        Some(wow_to_bevy([x, y, z]))
    }

    fn shape(&mut self) -> Option<Shape> {
        match self.u32()? {
            0 => Some(Shape::Circle),
            1 => Some(Shape::Cone),
            2 => Some(Shape::Line),
            3 => Some(Shape::Ring),
            _ => None,
        }
    }
}

fn mark(key: String, fields: &mut Fields, persistent_allowed: bool) -> Option<DrawCommand> {
    let shape = fields.shape()?;
    let at = fields.point()?;
    let yaw = fields.f32()?;
    let size = fields.f32()?;
    let duration = fields.secs()?;
    let flags = fields.u32().unwrap_or(0);
    let style = fields.u32().unwrap_or(0) as u8;
    let follow = fields.u64().unwrap_or(0);
    let texture = fields.next_str().filter(|t| !t.is_empty()).map(str::to_string);
    let done = fields.secs().unwrap_or(0.0);
    let opacity = fields.u32().map_or(1.0, |percent| percent.min(100) as f32 / 100.0);
    Some(DrawCommand::Mark(MarkSpec {
        key,
        shape,
        at,
        yaw,
        size,
        duration: (duration > 0.0 || !persistent_allowed).then_some(duration),
        outline_only: flags & OUTLINE_ONLY != 0,
        additive: flags & ADDITIVE != 0,
        ground_effect: flags & GROUND_EFFECT != 0,
        style,
        follow,
        texture,
        done,
        opacity,
    }))
}

pub fn parse_line(line: &str) -> Option<DrawCommand> {
    let mut fields = Fields(line.strip_prefix(DRAW_PREFIX)?.split('~'));
    match fields.next_str()? {
        "tg" => {
            let key = format!("{TELEGRAPH_KEY_PREFIX}{}", fields.u32()?);
            mark(key, &mut fields, false)
        }
        "tgx" => Some(DrawCommand::RemoveMark(format!("{TELEGRAPH_KEY_PREFIX}{}", fields.u32()?))),
        "tgc" => Some(DrawCommand::ClearTelegraphs),
        "clr" => Some(DrawCommand::ClearAll),
        "mark" => {
            let key = format!("{MARK_KEY_PREFIX}{}", fields.next_str()?);
            mark(key, &mut fields, true)
        }
        "markx" => Some(DrawCommand::RemoveMark(format!("{MARK_KEY_PREFIX}{}", fields.next_str()?))),
        "kit" => Some(DrawCommand::Kit(KitSpec {
            kit_id: fields.u32()?,
            at: fields.point()?,
            yaw: fields.f32()?,
            hold: fields.secs()?,
            raised: fields.u32().unwrap_or(0) == KIT_RAISED,
        })),
        "fly" => Some(DrawCommand::Fly(FlightSpec {
            id: fields.u32()?,
            from: fields.point()?,
            to: fields.point()?,
            duration: fields.secs()?,
            display_id: fields.u32()?,
            scale: fields.f32()?,
        })),
        "land" => Some(DrawCommand::Land(fields.u32()?)),
        "msl" => Some(DrawCommand::Missile(MissileSpec {
            spell_id: fields.u32()?,
            from: fields.point()?,
            to: fields.point()?,
            hold: fields.secs()?,
        })),
        "prop" => Some(DrawCommand::Prop(PropSpec {
            key: fields.u32()?,
            display_id: fields.u32()?,
            at: fields.point()?,
            yaw: fields.f32()?,
            scale: fields.f32().unwrap_or(1.0),
        })),
        "propx" => Some(DrawCommand::RemoveProp(fields.u32()?)),
        "aura" => Some(DrawCommand::Aura(AuraSpec {
            key: fields.u32()?,
            spell_id: fields.u32()?,
            at: fields.point()?,
            scale: fields.f32().unwrap_or(1.0),
        })),
        "aurax" => Some(DrawCommand::RemoveAura(fields.u32()?)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(line: &str) -> MarkSpec {
        match parse_line(line) {
            Some(DrawCommand::Mark(spec)) => spec,
            other => panic!("{line} is a mark, got {other:?}"),
        }
    }

    #[test]
    fn a_cone_telegraph_reads_every_field() {
        let spec = marked("HOA~tg~7~1~-8949.5~-132.25~83.5~1.5~10~2000~1~2");
        assert_eq!(spec.key, "t7");
        assert_eq!(spec.shape, Shape::Cone);
        assert_eq!(spec.at, wow_to_bevy([-8949.5, -132.25, 83.5]));
        assert_eq!((spec.yaw, spec.size, spec.duration), (1.5, 10.0, Some(2.0)));
        assert!(spec.outline_only);
        assert!(!spec.additive);
        assert_eq!(spec.style, 2);
        assert_eq!(spec.follow, 0);
        assert_eq!(spec.texture, None);
    }

    #[test]
    fn flags_and_style_are_optional() {
        let spec = marked("HOA~tg~1~0~0~0~0~0~4~1500");
        assert!(!spec.outline_only);
        assert_eq!(spec.style, 0);
        assert_eq!(spec.opacity, 1.0);
    }

    #[test]
    fn a_mark_can_be_drawn_faint() {
        let spec = marked("HOA~mark~reach~0~1~2~3~0~10~0~1~3~4294967297~~0~40");
        assert_eq!(spec.opacity, 0.4);
        assert_eq!(marked("HOA~mark~reach~0~1~2~3~0~10~0~1~3~0~~0~250").opacity, 1.0);
    }

    #[test]
    fn a_mark_without_time_stays_and_can_follow_and_wear_a_texture() {
        let spec = marked("HOA~mark~ley3~0~1~2~3~0.5~5~0~2~1~4294967297~World\\Goober\\RuneGroundBlue01.blp");
        assert_eq!(spec.key, "mley3");
        assert_eq!(spec.duration, None);
        assert!(spec.additive);
        assert_eq!(spec.follow, 4_294_967_297);
        assert_eq!(spec.texture.as_deref(), Some("World\\Goober\\RuneGroundBlue01.blp"));
        assert_eq!(spec.done, 0.0);
        assert_eq!(parse_line("HOA~markx~ley3"), Some(DrawCommand::RemoveMark("mley3".into())));
    }

    #[test]
    fn a_resumed_mark_carries_the_time_already_filled() {
        let spec = marked("HOA~mark~hearth~0~1~2~3~0~5~15000~0~3~0~~4500");
        assert_eq!(spec.duration, Some(15.0));
        assert_eq!(spec.texture, None);
        assert_eq!(spec.done, 4.5);
    }

    #[test]
    fn a_kit_is_on_the_ground_unless_the_server_says_it_is_raised() {
        let kit = |line: &str| match parse_line(line) {
            Some(DrawCommand::Kit(kit)) => kit,
            other => panic!("{line} is a kit, got {other:?}"),
        };
        let grounded = kit("HOA~kit~92~1~2~3~0.5~1500");
        assert_eq!(grounded.at, wow_to_bevy([1.0, 2.0, 3.0]));
        assert_eq!(grounded.hold, 1.5);
        assert!(!grounded.raised);
        assert!(kit("HOA~kit~92~1~2~3~0.5~1500~1").raised);
    }

    #[test]
    fn a_flight_reads_both_ends_and_its_look() {
        assert_eq!(
            parse_line("HOA~fly~3~1~2~3~4~5~6~750~18600~1.25"),
            Some(DrawCommand::Fly(FlightSpec {
                id: 3,
                from: wow_to_bevy([1.0, 2.0, 3.0]),
                to: wow_to_bevy([4.0, 5.0, 6.0]),
                duration: 0.75,
                display_id: 18600,
                scale: 1.25,
            }))
        );
    }

    #[test]
    fn a_missile_reads_its_spell_both_points_and_hold() {
        assert_eq!(
            parse_line("HOA~msl~13241~1~2~3~4~5~6~1500"),
            Some(DrawCommand::Missile(MissileSpec {
                spell_id: 13241,
                from: wow_to_bevy([1.0, 2.0, 3.0]),
                to: wow_to_bevy([4.0, 5.0, 6.0]),
                hold: 1.5,
            }))
        );
    }

    #[test]
    fn a_prop_reads_its_key_display_and_place() {
        assert_eq!(
            parse_line("HOA~prop~9~17986~1~2~3~0.5~1.25"),
            Some(DrawCommand::Prop(PropSpec {
                key: 9,
                display_id: 17986,
                at: wow_to_bevy([1.0, 2.0, 3.0]),
                yaw: 0.5,
                scale: 1.25,
            }))
        );
        assert_eq!(parse_line("HOA~propx~9"), Some(DrawCommand::RemoveProp(9)));
    }

    #[test]
    fn other_lines_and_broken_lines_are_ignored() {
        assert_eq!(parse_line("BONK~hearth~1"), None);
        assert_eq!(parse_line("HOA~tg~1~9~0~0~0~0~4~1500"), None);
        assert_eq!(parse_line("HOA~kit~92~1~2"), None);
        assert_eq!(parse_line("HOA~tgx~12"), Some(DrawCommand::RemoveMark("t12".into())));
    }
}
