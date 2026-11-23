//! Sound playback (`FUN_00402ba0`'s `sndPlaySoundA(data, SND_ASYNC |
//! SND_MEMORY)`): one sound at a time, and a new one cuts off whatever is
//! playing.
//!
//! The sounds are the WAVE resources the shipped `ski32.exe` lacks. When
//! `tools/extract_assets.py` found them (in `ski32sounds.exe`), build.rs
//! embeds `assets/sounds/sound_1..9.wav` and sets `cfg(skifree_sound)`;
//! without them nothing here plays and no audio code is compiled.
//!
//! Backends, none of which needs anything at build time:
//! - Windows: `sndPlaySoundA` itself, with the same bytes and flags.
//! - macOS: one `NSSound` per WAV; `stop` then `play`.
//! - Linux: libasound loaded at runtime (like libX11 in os_window), fed by
//!   a player thread; `snd_pcm_drop` cuts.
//! - Web: Web Audio through `web/skifree.js`. The WAVs come with the
//!   `AssetSet` (embedded in the private build, extracted from the
//!   player's `ski32sounds.exe` in the public one), so build.rs's
//!   `cfg(skifree_sound)` is not used there.
//!
//! Muting (the port's M key; the original's `nosound` argument starts the
//! game muted) stops the current sound and drops later ones.

use skifree::extract::AssetSet;
use skifree::game::sound::Sound;

pub struct Audio {
    muted: bool,
    #[cfg(any(skifree_sound, target_arch = "wasm32"))]
    player: Option<imp::Player>,
}

impl Audio {
    pub fn new(muted: bool, assets: &AssetSet) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = assets;
        Self {
            muted,
            #[cfg(skifree_sound)]
            player: imp::Player::open(),
            #[cfg(target_arch = "wasm32")]
            player: imp::Player::open(assets),
        }
    }

    /// Play one tick's sounds: each cuts off the one before, so only the
    /// last is heard (the earlier ones would last microseconds).
    pub fn play_tick(&mut self, sounds: &[Sound]) {
        if let Some(&last) = sounds.last() {
            self.play(last);
        }
    }

    pub fn play(&mut self, sound: Sound) {
        if self.muted {
            return;
        }
        #[cfg(any(skifree_sound, target_arch = "wasm32"))]
        if let Some(p) = self.player.as_mut() {
            p.play(sound);
        }
        #[cfg(not(any(skifree_sound, target_arch = "wasm32")))]
        let _ = sound;
    }

    /// Stop whatever is playing (`sndPlaySoundA(NULL, 0)`, as the original
    /// does on exit).
    pub fn stop(&mut self) {
        #[cfg(any(skifree_sound, target_arch = "wasm32"))]
        if let Some(p) = self.player.as_mut() {
            p.stop();
        }
    }

    /// The M key: mute (stopping the current sound) or unmute.
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        if self.muted {
            self.stop();
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The nine WAVs, by resource id - 1.
#[cfg(all(skifree_sound, not(skifree_no_assets)))]
static WAVS: [&[u8]; 9] = [
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_1.wav")),
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_2.wav")),
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_3.wav")),
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_4.wav")),
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_5.wav")),
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_6.wav")),
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_7.wav")),
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_8.wav")),
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sounds/sound_9.wav")),
];

/// `SKIFREE_NO_ASSETS=1` (build.rs): compile, lint and test the sound code
/// without the WAVs. This build plays nothing.
#[cfg(all(skifree_sound, skifree_no_assets))]
static WAVS: [&[u8]; 9] = [&[]; 9];

#[cfg(all(skifree_sound, any(target_os = "windows", target_os = "linux")))]
fn wav(sound: Sound) -> &'static [u8] {
    WAVS[sound.id() as usize - 1]
}

#[cfg(all(skifree_sound, target_os = "windows"))]
mod imp {
    use super::wav;
    use skifree::game::sound::Sound;

    const SND_ASYNC: u32 = 0x1;
    const SND_MEMORY: u32 = 0x4;

    #[link(name = "winmm")]
    extern "system" {
        fn sndPlaySoundA(sound: *const u8, flags: u32) -> i32;
    }

    pub struct Player;

    impl Player {
        pub fn open() -> Option<Self> {
            Some(Player)
        }

        /// Exactly the original's call (FUN_00402ba0). The data is static,
        /// so it outlives the asynchronous playback.
        pub fn play(&mut self, sound: Sound) {
            unsafe { sndPlaySoundA(wav(sound).as_ptr(), SND_ASYNC | SND_MEMORY) };
        }

        pub fn stop(&mut self) {
            unsafe { sndPlaySoundA(std::ptr::null(), 0) };
        }
    }
}

#[cfg(all(skifree_sound, target_os = "macos"))]
mod imp {
    use super::WAVS;
    use macroquad::miniquad::native::apple::frameworks::{class, msg_send, sel, sel_impl, ObjcId, NO};
    use skifree::game::sound::Sound;

    pub struct Player {
        sounds: Vec<ObjcId>,
        current: Option<usize>,
    }

    impl Player {
        pub fn open() -> Option<Self> {
            let mut sounds = Vec::new();
            for bytes in WAVS {
                unsafe {
                    let data: ObjcId = msg_send![class!(NSData),
                        dataWithBytesNoCopy: bytes.as_ptr() as *mut std::ffi::c_void
                        length: bytes.len()
                        freeWhenDone: NO];
                    let sound: ObjcId = msg_send![class!(NSSound), alloc];
                    let sound: ObjcId = msg_send![sound, initWithData: data];
                    if sound.is_null() {
                        return None;
                    }
                    sounds.push(sound);
                }
            }
            Some(Player { sounds, current: None })
        }

        pub fn play(&mut self, sound: Sound) {
            self.stop();
            let i = sound.id() as usize - 1;
            unsafe {
                // `play` refuses a sound that is still playing; it was
                // stopped above, so this restarts it from the start.
                let _: bool = msg_send![self.sounds[i], play];
            }
            self.current = Some(i);
        }

        pub fn stop(&mut self) {
            if let Some(i) = self.current.take() {
                unsafe {
                    let _: bool = msg_send![self.sounds[i], stop];
                }
            }
        }
    }
}

#[cfg(all(skifree_sound, target_os = "linux"))]
mod imp {
    use super::wav;
    use skifree::game::sound::Sound;
    use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
    use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

    type Pcm = c_void;
    const SND_PCM_STREAM_PLAYBACK: c_int = 0;
    const SND_PCM_FORMAT_U8: c_int = 1;
    const SND_PCM_FORMAT_S16_LE: c_int = 2;
    const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;
    /// Frames written per call, so a new sound cuts in within one chunk.
    const CHUNK: usize = 256;

    struct Alsa {
        open: unsafe extern "C" fn(*mut *mut Pcm, *const c_char, c_int, c_int) -> c_int,
        set_params: unsafe extern "C" fn(*mut Pcm, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int,
        writei: unsafe extern "C" fn(*mut Pcm, *const c_void, c_ulong) -> c_long,
        recover: unsafe extern "C" fn(*mut Pcm, c_int, c_int) -> c_int,
        drop: unsafe extern "C" fn(*mut Pcm) -> c_int,
    }

    impl Alsa {
        unsafe fn load() -> Option<Alsa> {
            let lib = libc::dlopen(c"libasound.so.2".as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
            if lib.is_null() {
                return None;
            }
            macro_rules! sym {
                ($name:literal) => {{
                    let p = libc::dlsym(lib, concat!($name, "\0").as_ptr() as *const c_char);
                    if p.is_null() {
                        return None;
                    }
                    std::mem::transmute(p)
                }};
            }
            Some(Alsa {
                open: sym!("snd_pcm_open"),
                set_params: sym!("snd_pcm_set_params"),
                writei: sym!("snd_pcm_writei"),
                recover: sym!("snd_pcm_recover"),
                drop: sym!("snd_pcm_drop"),
            })
        }
    }

    /// A WAV's PCM format and samples.
    struct WavPcm {
        format: c_int,
        channels: c_uint,
        rate: c_uint,
        frame_bytes: usize,
        data: &'static [u8],
    }

    fn parse_wav(bytes: &'static [u8]) -> Option<WavPcm> {
        let u16_at = |o: usize| Some(u16::from_le_bytes(bytes.get(o..o + 2)?.try_into().ok()?));
        let u32_at = |o: usize| Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?));
        if bytes.get(0..4)? != b"RIFF" || bytes.get(8..12)? != b"WAVE" {
            return None;
        }
        let (mut fmt, mut data) = (None, None);
        let mut o = 12;
        while o + 8 <= bytes.len() {
            let len = u32_at(o + 4)? as usize;
            let body = o + 8;
            match bytes.get(o..o + 4)? {
                b"fmt " => fmt = Some(body),
                b"data" => data = Some(bytes.get(body..(body + len).min(bytes.len()))?),
                _ => {}
            }
            o = body + len + (len & 1);
        }
        let f = fmt?;
        if u16_at(f)? != 1 {
            return None; // not PCM
        }
        let channels = u16_at(f + 2)? as c_uint;
        let rate = u32_at(f + 4)?;
        let bits = u16_at(f + 14)?;
        let format = match bits {
            8 => SND_PCM_FORMAT_U8,
            16 => SND_PCM_FORMAT_S16_LE,
            _ => return None,
        };
        Some(WavPcm { format, channels, rate, frame_bytes: channels as usize * bits as usize / 8, data: data? })
    }

    enum Cmd {
        Play(Sound),
        Stop,
    }

    pub struct Player {
        tx: Sender<Cmd>,
    }

    impl Player {
        pub fn open() -> Option<Self> {
            let alsa = unsafe { Alsa::load() }?;
            let mut pcm: *mut Pcm = std::ptr::null_mut();
            if unsafe { (alsa.open)(&mut pcm, c"default".as_ptr(), SND_PCM_STREAM_PLAYBACK, 0) } < 0 {
                return None;
            }
            let pcm = pcm as usize; // moved to the player thread
            let (tx, rx) = channel();
            std::thread::Builder::new()
                .name("skifree-audio".into())
                .spawn(move || run(alsa, pcm as *mut Pcm, rx))
                .ok()?;
            Some(Player { tx })
        }

        pub fn play(&mut self, sound: Sound) {
            let _ = self.tx.send(Cmd::Play(sound));
        }

        pub fn stop(&mut self) {
            let _ = self.tx.send(Cmd::Stop);
        }
    }

    fn run(alsa: Alsa, pcm: *mut Pcm, rx: Receiver<Cmd>) {
        let mut next = None;
        loop {
            let cmd = match next.take() {
                Some(c) => c,
                None => match rx.recv() {
                    Ok(c) => c,
                    Err(_) => return,
                },
            };
            unsafe { (alsa.drop)(pcm) };
            let Cmd::Play(sound) = cmd else { continue };
            let Some(w) = parse_wav(wav(sound)) else { continue };
            // 100 ms of buffer, resampled by ALSA if the device needs it.
            let r = unsafe {
                (alsa.set_params)(pcm, w.format, SND_PCM_ACCESS_RW_INTERLEAVED, w.channels, w.rate, 1, 100_000)
            };
            if r < 0 {
                continue;
            }
            for chunk in w.data.chunks(CHUNK * w.frame_bytes) {
                match rx.try_recv() {
                    Ok(c) => {
                        next = Some(c);
                        break;
                    }
                    Err(TryRecvError::Disconnected) => return,
                    Err(TryRecvError::Empty) => {}
                }
                let frames = (chunk.len() / w.frame_bytes) as c_ulong;
                let n = unsafe { (alsa.writei)(pcm, chunk.as_ptr() as *const c_void, frames) };
                if n < 0 && unsafe { (alsa.recover)(pcm, n as c_int, 1) } < 0 {
                    break;
                }
            }
            // The tail drains on its own; the next command drops it.
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use crate::platform::web;
    use skifree::extract::AssetSet;
    use skifree::game::sound::Sound;

    /// The page holds the nine sounds (as Web Audio buffers, see
    /// web/skifree.js); a new one stops the one playing, like
    /// sndPlaySoundA. Browsers start audio only after the player's first key
    /// press or click: the page drops sounds before that.
    pub struct Player;

    impl Player {
        pub fn open(assets: &AssetSet) -> Option<Self> {
            let wavs = assets.sounds.as_ref()?;
            for (i, wav) in wavs.iter().enumerate() {
                web::audio_load(i as u32, wav);
            }
            Some(Player)
        }

        pub fn play(&mut self, sound: Sound) {
            web::audio_play(sound.id() as u32 - 1);
        }

        pub fn stop(&mut self) {
            web::audio_stop();
        }
    }
}

#[cfg(all(
    skifree_sound,
    not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_arch = "wasm32"))
))]
mod imp {
    use skifree::game::sound::Sound;

    pub struct Player;

    impl Player {
        pub fn open() -> Option<Self> {
            None
        }
        pub fn play(&mut self, _: Sound) {}
        pub fn stop(&mut self) {}
    }
}
