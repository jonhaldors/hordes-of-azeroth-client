use benilla_assets::coords::wow_to_bevy;
use bevy::prelude::Vec3;

pub const DRAW_PREFIX: &str = "HOA~";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Circle,
    Cone,
    Line,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelegraphSpec {
    pub id: u32,
    pub shape: Shape,
    pub at: Vec3,
    pub yaw: f32,
    pub size: f32,
    pub duration: f32,
    pub outline_only: bool,
    pub style: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KitSpec {
    pub kit_id: u32,
    pub at: Vec3,
    pub yaw: f32,
    pub hold: f32,
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

#[derive(Clone, Debug, PartialEq)]
pub enum DrawCommand {
    Telegraph(TelegraphSpec),
    RemoveTelegraph(u32),
    ClearTelegraphs,
    Kit(KitSpec),
    Fly(FlightSpec),
    Land(u32),
}

const OUTLINE_ONLY: u32 = 1;

struct Fields<'a>(std::str::Split<'a, char>);

impl Fields<'_> {
    fn next_str(&mut self) -> Option<&str> {
        self.0.next()
    }

    fn u32(&mut self) -> Option<u32> {
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
}

pub fn parse_line(line: &str) -> Option<DrawCommand> {
    let mut fields = Fields(line.strip_prefix(DRAW_PREFIX)?.split('~'));
    match fields.next_str()? {
        "tg" => {
            let id = fields.u32()?;
            let shape = match fields.u32()? {
                0 => Shape::Circle,
                1 => Shape::Cone,
                2 => Shape::Line,
                _ => return None,
            };
            let at = fields.point()?;
            let yaw = fields.f32()?;
            let size = fields.f32()?;
            let duration = fields.secs()?;
            let flags = fields.u32().unwrap_or(0);
            let style = fields.u32().unwrap_or(0) as u8;
            Some(DrawCommand::Telegraph(TelegraphSpec {
                id,
                shape,
                at,
                yaw,
                size,
                duration,
                outline_only: flags & OUTLINE_ONLY != 0,
                style,
            }))
        }
        "tgx" => Some(DrawCommand::RemoveTelegraph(fields.u32()?)),
        "tgc" => Some(DrawCommand::ClearTelegraphs),
        "kit" => Some(DrawCommand::Kit(KitSpec {
            kit_id: fields.u32()?,
            at: fields.point()?,
            yaw: fields.f32()?,
            hold: fields.secs()?,
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
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cone_telegraph_reads_every_field() {
        let Some(DrawCommand::Telegraph(spec)) =
            parse_line("HOA~tg~7~1~-8949.5~-132.25~83.5~1.5~10~2000~1~2")
        else {
            panic!("a telegraph line parses");
        };
        assert_eq!(spec.id, 7);
        assert_eq!(spec.shape, Shape::Cone);
        assert_eq!(spec.at, wow_to_bevy([-8949.5, -132.25, 83.5]));
        assert_eq!((spec.yaw, spec.size, spec.duration), (1.5, 10.0, 2.0));
        assert!(spec.outline_only);
        assert_eq!(spec.style, 2);
    }

    #[test]
    fn flags_and_style_are_optional() {
        let Some(DrawCommand::Telegraph(spec)) = parse_line("HOA~tg~1~0~0~0~0~0~4~1500") else {
            panic!("a short telegraph line parses");
        };
        assert!(!spec.outline_only);
        assert_eq!(spec.style, 0);
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
    fn other_lines_and_broken_lines_are_ignored() {
        assert_eq!(parse_line("BONK~hearth~1"), None);
        assert_eq!(parse_line("HOA~tg~1~9~0~0~0~0~4~1500"), None);
        assert_eq!(parse_line("HOA~kit~92~1~2"), None);
        assert_eq!(parse_line("HOA~tgx~12"), Some(DrawCommand::RemoveTelegraph(12)));
    }
}
