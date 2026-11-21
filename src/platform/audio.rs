/// Audio stub — no WAV resources exist in the original binary.
/// The code references 9 WAVE resources but FindResourceA returns NULL for all of them.
/// If replacement sounds are added later, drop them into assets/sounds/ and load here.

pub struct Audio;

impl Audio {
    pub fn new() -> Self {
        Self
    }

    pub fn load_sounds(&self) {
        // no-op: no sound resources in the original binary
    }

    #[allow(dead_code)] // Phase 3
    pub fn play_sound(&self, _id: u32) {
        // no-op
    }
}
