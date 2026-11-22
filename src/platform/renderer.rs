use macroquad::prelude::*;
use std::collections::HashMap;

pub struct Renderer {
    sprites: HashMap<u32, Texture2D>,
}

impl Renderer {
    pub async fn new() -> Self {
        let mut sprites = HashMap::new();
        let root = skifree::game::assets::asset_root();
        for id in 1..=89u32 {
            let path = root.join(format!("sprites/bitmap_{:02}.png", id));
            let path_str = path.to_string_lossy().into_owned();
            match load_texture(&path_str).await {
                Ok(tex) => {
                    sprites.insert(id, tex);
                }
                Err(e) => {
                    eprintln!("warning: failed to load sprite {}: {}", path_str, e);
                }
            }
        }
        Self { sprites }
    }

    pub fn draw_sprite(&self, id: u32, x: f32, y: f32) {
        if let Some(tex) = self.sprites.get(&id) {
            draw_texture(tex, x, y, WHITE);
        }
    }

    #[allow(dead_code)]
    pub fn draw_sprite_scaled(&self, id: u32, x: f32, y: f32, scale: f32) {
        if let Some(tex) = self.sprites.get(&id) {
            draw_texture_ex(
                tex,
                x,
                y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(tex.width() * scale, tex.height() * scale)),
                    ..Default::default()
                },
            );
        }
    }

    #[allow(dead_code)]
    pub fn sprite_count(&self) -> usize {
        self.sprites.len()
    }
}
