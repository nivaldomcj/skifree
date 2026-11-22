/// Actor behavior classes (param_1[6] in the original)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ActorClass {
    Skier = 0,
    Dog = 1,
    Snowboarder = 2,
    Obstacle = 3,
    AmbientMob = 4,
    MonsterLeft = 5,
    MonsterRight = 6,
    MonsterTop = 7,
    MonsterBottom = 8,
    Eating = 9,
    Lift = 10,
    /// Class 0x11: inert visual effect (e.g. the snowboarder jump puff).
    /// The original gives classes >= 0xb no behavior update; the actor just
    /// holds its raw sprite until it scrolls out and is culled.
    Effect = 17,
}

impl ActorClass {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0 => Some(Self::Skier),
            1 => Some(Self::Dog),
            2 => Some(Self::Snowboarder),
            3 => Some(Self::Obstacle),
            4 => Some(Self::AmbientMob),
            5 => Some(Self::MonsterLeft),
            6 => Some(Self::MonsterRight),
            7 => Some(Self::MonsterTop),
            8 => Some(Self::MonsterBottom),
            9 => Some(Self::Eating),
            10 => Some(Self::Lift),
            17 => Some(Self::Effect),
            _ => None,
        }
    }
}

/// Actor flags (bitfield at +0x4c in the original)
#[derive(Debug, Clone, Copy, Default)]
pub struct ActorFlags {
    pub active: bool,     // bit 0
    pub dead: bool,       // bit 1
    pub visible: bool,    // bit 2
    pub on_screen: bool,  // bit 3
    pub no_clip: bool,    // bit 4
    pub new_sprite: bool, // bit 5
}

/// One actor in the game world (mirrors the 80-byte original struct)
#[derive(Debug, Clone)]
pub struct Actor {
    pub active: bool,
    pub class: ActorClass,
    pub state: u16,
    pub sprite_id: u16,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
    pub flags: ActorFlags,
    pub timestamp: u32,
    pub event_idx: Option<usize>,
    pub next: Option<usize>,
}

impl Default for Actor {
    fn default() -> Self {
        Self {
            active: false,
            class: ActorClass::Skier,
            state: 0,
            sprite_id: 0,
            x: 0,
            y: 0,
            z: 0,
            vx: 0,
            vy: 0,
            vz: 0,
            flags: ActorFlags::default(),
            timestamp: 0,
            event_idx: None,
            next: None,
        }
    }
}

/// A request to spawn a secondary actor, returned by behavior updates that
/// create actors mid-tick (the ambient mob's 1-in-1000 crossing skier, the
/// snowboarder's jump effect). Exactly one of `state` / `sprite` is set:
/// `state` goes through the state-to-sprite table (FUN_004020d0), `sprite`
/// sets a raw sprite id (FUN_004026a0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnRequest {
    pub class: ActorClass,
    pub state: Option<u16>,
    pub sprite: Option<u16>,
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// Spawn event — consumed as the player scrolls past its y coordinate
#[derive(Debug, Clone)]
pub struct SpawnEvent {
    pub actor_idx: Option<usize>,
    pub event_type: u16, // 0x11=marker, 0x0c=gate, 0x0d=obstacle, 0x04=mob, 5-8=monster
    pub state: u16,
    pub class: u16,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub param: i16,
}

/// Camera / viewport state
#[derive(Debug, Clone, Default)]
pub struct Camera {
    pub cx: i16,
    pub cy: i16,
    pub scroll_x: i16,
    pub scroll_y: i16,
}

/// Course type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourseType {
    Freestyle,
    Slalom,
    TreeSlalom,
}
