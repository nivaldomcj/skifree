//! The original's sound effects (`FUN_00402ba0`).
//!
//! `FUN_004052d0` loads WAVE resources 1-9 into nine slots at startup
//! (`FUN_00405640`), and each event below plays its slot with
//! `sndPlaySoundA(data, SND_ASYNC | SND_MEMORY)`: one sound at a time, so a
//! new one cuts off whatever is playing. The shipped `ski32.exe` lacks the
//! resources, so the play routine returns before reaching `sndPlaySoundA`,
//! but it is still called at every event. The game logic only records the
//! calls; the platform layer plays them (docs/how-ski32-works.md, "Sound").

/// One sound, by its WAVE resource id (the slot `FUN_00402ba0` gets in ecx).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Sound {
    /// A hit that costs 32 style (0x403e48) or a crash landing (0x402aeb).
    Crash = 1,
    /// The skier takes off: the ramp, the hop over a large rock, an
    /// airborne bounce off a rock (0x403be8 / 0x403ccb).
    Launch = 2,
    /// A snowboarder knocked down by anything moving (0x403fa8).
    SnowboarderHit = 3,
    /// A clean landing (0x402aeb).
    Land = 4,
    /// A crossing skier launched by a bump or a ramp (0x403ef3).
    SkierLaunch = 5,
    /// A dog knocked over, by anything (0x403f5f).
    Dog = 6,
    /// The monster catches the player (0x403b1e).
    Eaten = 7,
    /// A snowboarder jumps (0x40371a).
    SnowboarderJump = 8,
    /// The monster chases the player, every tick (0x4045d3).
    Chase = 9,
}

impl Sound {
    pub const ALL: [Sound; 9] = [
        Sound::Crash,
        Sound::Launch,
        Sound::SnowboarderHit,
        Sound::Land,
        Sound::SkierLaunch,
        Sound::Dog,
        Sound::Eaten,
        Sound::SnowboarderJump,
        Sound::Chase,
    ];

    /// The WAVE resource id.
    pub fn id(self) -> u8 {
        self as u8
    }
}
