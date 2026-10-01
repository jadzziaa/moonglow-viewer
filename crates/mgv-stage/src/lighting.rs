//! Lighting rigs: what lights the stage besides the models' own lights: a
//! neutral studio light, or an area's light (an `environment.2da` preset
//! by day or night, or settings typed in as an area holds them).

use glam::Vec3;
use mg_render::{AreaLight, Fog, PointLight};

/// The scene-wide light: sun or moon, fog, background, extra lights and the
/// environment map.
#[derive(Debug, Clone, PartialEq)]
pub struct Lighting {
    pub area: AreaLight,
    pub fog: Option<Fog>,
    /// Clear colour, gamma space.
    pub background: [f32; 3],
    /// Lights placed in the scene besides the models' own.
    pub lights: Vec<PointLight>,
    /// For textures asking for the `default` environment map (`chrome1`
    /// when `None`).
    pub env_map: Option<String>,
    /// Tile main lights 1 and 2 (`…ml1`, `…ml2` light nodes), as
    /// `lightcolor.2da` colours; `None` leaves the node's own colour.
    pub main_lights: [Option<Vec3>; 2],
}

impl Lighting {
    /// A neutral light for looking at models: the renderer's preview
    /// daylight (the game's default sun direction) on a dark blue-grey.
    pub fn studio() -> Lighting {
        Lighting {
            area: AreaLight::default(),
            fog: None,
            background: [0.16, 0.18, 0.21],
            lights: Vec::new(),
            env_map: None,
            main_lights: [None, None],
        }
    }
}

impl Default for Lighting {
    fn default() -> Lighting {
        Lighting::studio()
    }
}

/// An area's light settings as an area holds them (ARE fields), for the
/// area rigs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaSettings {
    /// Sun or moon colours, 0–255 RGB (gamma space).
    pub ambient: [u8; 3],
    pub diffuse: [u8; 3],
    pub fog_color: [u8; 3],
    /// `FogAmount`: metres the fog starts nearer than 30.
    pub fog_amount: u8,
    /// `FogClipDist`: where the fog is complete.
    pub fog_clip: f32,
    /// Tile main lights 1 and 2: `lightcolor.2da` rows.
    pub main_lights: [u8; 2],
    pub fog: bool,
}

impl Default for AreaSettings {
    /// `environment.2da`'s ExteriorClear by day (the area wizard's
    /// default), with Aurora's fog clip distance.
    fn default() -> AreaSettings {
        AreaSettings {
            ambient: [50, 50, 100],
            diffuse: [255, 255, 255],
            fog_color: [104, 126, 145],
            fog_amount: 0,
            fog_clip: 45.0,
            main_lights: [0, 0],
            fog: false,
        }
    }
}

/// An ARE colour (0x00BBGGRR) from RGB bytes.
fn bgr(c: [u8; 3]) -> u32 {
    u32::from(c[0]) | u32::from(c[1]) << 8 | u32::from(c[2]) << 16
}

/// The game's default sun and moon direction (towards 4000, 4500, 7000).
pub fn sun_direction() -> glam::Vec3 {
    glam::Vec3::new(4000.0, 4500.0, 7000.0).normalize()
}

impl Lighting {
    /// An area's light, as the client sets it: sun or moon, the fog
    /// (ending at `fog_clip`, starting `fog_amount` metres nearer than
    /// 30 m; the toolset's `mg_area::scene::fog`, checked against the
    /// client) and its colour behind everything, tile main lights coloured
    /// from `lightcolor.2da`.
    pub fn area(s: &AreaSettings, lib: &mgv_library::Library) -> Lighting {
        let fog_color = Vec3::from(s.fog_color.map(|c| f32::from(c) / 255.0));
        let main = |row: u8| -> Option<Vec3> {
            if row == 0 {
                return None;
            }
            let t = lib.game().table("lightcolor").ok()?;
            let f = |c: &str| t.get(usize::from(row), c).and_then(mg_2da::parse_float);
            Some(Vec3::new(f("RED")?, f("GREEN")?, f("BLUE")?))
        };
        let end = s.fog_clip.max(1.0);
        Lighting {
            area: AreaLight::from_are(bgr(s.ambient), bgr(s.diffuse), sun_direction()),
            fog: s.fog.then(|| Fog {
                start: (30.0 - f32::from(s.fog_amount)).min(end - 1.0),
                end,
                color: fog_color,
            }),
            background: fog_color.to_array(),
            lights: Vec::new(),
            env_map: None,
            main_lights: [main(s.main_lights[0]), main(s.main_lights[1])],
        }
    }
}

/// An `environment.2da` preset (the area wizard's lighting schemes).
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    pub row: usize,
    pub label: String,
    pub day: AreaSettings,
    pub night: AreaSettings,
}

/// The presets of `environment.2da`.
pub fn environments(lib: &mgv_library::Library) -> Vec<Environment> {
    let Ok(t) = lib.game().table("environment") else { return Vec::new() };
    (0..t.len())
        .filter_map(|row| {
            let label = t.get(row, "LABEL").filter(|l| *l != "****")?.to_string();
            let byte = |c: &str| {
                t.get(row, c).and_then(mg_2da::parse_int).map_or(0, |v| v.clamp(0, 255) as u8)
            };
            let rgb = |p: &str| {
                [byte(&format!("{p}_RED")), byte(&format!("{p}_GREEN")), byte(&format!("{p}_BLUE"))]
            };
            let settings = |light: bool| {
                let (pre, fog) = if light { ("LIGHT", "LIGHT_FOG") } else { ("DARK", "DARK_FOG") };
                AreaSettings {
                    ambient: rgb(&format!("{pre}_AMB")),
                    diffuse: rgb(&format!("{pre}_DIFF")),
                    fog_color: rgb(fog),
                    fog_amount: byte(fog),
                    fog_clip: 45.0,
                    main_lights: [byte("MAIN1_COLOR1"), byte("MAIN2_COLOR1")],
                    fog: false,
                }
            };
            Some(Environment { row, label, day: settings(true), night: settings(false) })
        })
        .collect()
}
