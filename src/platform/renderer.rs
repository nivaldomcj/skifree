use macroquad::prelude::*;
use std::collections::HashMap;

pub struct Renderer {
    sprites: HashMap<u32, Texture2D>,
    /// Status display font: printable ASCII in 8x12 cells from 0x20. The
    /// stock OEM fixed font ("Terminal") extracted from vgaoem.fon
    /// (assets/fonts/oem_fixed.png), or the bundled fallback.
    font: Texture2D,
}

/// Status display geometry, from the original's text metrics
/// (FUN_00406a70): 8x12 cells, label column = widest label ("Speed:"),
/// value column = widest value template ("00:00:00.00"), 2 px margins.
const CELL_W: f32 = 8.0;
/// IBM VGA 8x14 cut to 8x12, from The Ultimate Oldschool PC Font Pack by
/// VileR (CC BY-SA 4.0; see resources/fonts/README.md).
const FALLBACK_FONT: &[u8] = include_bytes!("../../resources/fonts/ibm_vga_8x12.png");
const CELL_H: f32 = 12.0;
const LABELS: [&str; 4] = ["Time:", "Dist:", "Speed:", "Style:"];
const VALUE_TEMPLATE_CHARS: f32 = 11.0;

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
        // Terminal extracted from the user's vgaoem.fon if present, else the
        // bundled IBM VGA fallback (resources/fonts, CC BY-SA 4.0).
        let font_path = root.join("fonts/oem_fixed.png");
        let font = match load_texture(&font_path.to_string_lossy()).await {
            Ok(tex) => tex,
            Err(_) => Texture2D::from_file_with_format(FALLBACK_FONT, Some(ImageFormat::Png)),
        };
        font.set_filter(FilterMode::Nearest);
        Self { sprites, font }
    }

    /// Draw `text` in the OEM fixed font with its top-left cell at (x, y),
    /// black on an opaque white background (TextOutA's default OPAQUE mode).
    pub fn draw_oem_text(&self, text: &str, x: f32, y: f32) {
        let n = text.chars().count() as f32;
        draw_rectangle(x, y, n * CELL_W, CELL_H, WHITE);
        for (i, c) in text.chars().enumerate() {
            let code = c as u32;
            if !(0x20..0x7f).contains(&code) {
                continue;
            }
            draw_texture_ex(
                &self.font,
                x + i as f32 * CELL_W,
                y,
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new((code - 0x20) as f32 * CELL_W, 0.0, CELL_W, CELL_H)),
                    ..Default::default()
                },
            );
        }
    }

    /// A one-line notice in the status display's style, centred in the
    /// client area: white box, one-pixel black frame, 2 px margins.
    pub fn draw_notice(&self, text: &str, client_w: f32, client_h: f32) {
        let w = text.chars().count() as f32 * CELL_W + 4.0;
        let h = CELL_H + 4.0;
        let x0 = ((client_w - w) / 2.0).floor();
        let y0 = ((client_h - h) / 2.0).floor();
        draw_rectangle(x0, y0, w, 1.0, BLACK);
        draw_rectangle(x0, y0 + h - 1.0, w, 1.0, BLACK);
        draw_rectangle(x0, y0, 1.0, h, BLACK);
        draw_rectangle(x0 + w - 1.0, y0, 1.0, h, BLACK);
        draw_rectangle(x0 + 1.0, y0 + 1.0, w - 2.0, h - 2.0, WHITE);
        self.draw_oem_text(text, x0 + 2.0, y0 + 2.0);
    }

    /// The high-score message box where the platform has no native one:
    /// in the status display's style (OEM font, white box, one-pixel black
    /// frames), centred: the caption, a rule, the text lines and an OK
    /// button. Returns the button's rectangle (x, y, w, h).
    pub fn draw_message_box(&self, caption: &str, text: &str, client_w: f32, client_h: f32) -> (f32, f32, f32, f32) {
        let lines: Vec<&str> = text.split('\n').collect();
        let cols = lines.iter().chain([&caption]).map(|l| l.chars().count()).max().unwrap_or(0) as f32;
        let (pad, btn_w, btn_h) = (8.0, 8.0 * CELL_W, CELL_H + 8.0);
        let w = (cols * CELL_W + 2.0 * pad).max(btn_w + 2.0 * pad);
        let h = CELL_H + 6.0 + lines.len() as f32 * CELL_H + 2.0 * pad + btn_h + pad;
        let x0 = ((client_w - w) / 2.0).floor();
        let y0 = ((client_h - h) / 2.0).floor();
        let frame = |x: f32, y: f32, w: f32, h: f32| {
            draw_rectangle(x, y, w, h, WHITE);
            draw_rectangle(x, y, w, 1.0, BLACK);
            draw_rectangle(x, y + h - 1.0, w, 1.0, BLACK);
            draw_rectangle(x, y, 1.0, h, BLACK);
            draw_rectangle(x + w - 1.0, y, 1.0, h, BLACK);
        };
        frame(x0, y0, w, h);
        self.draw_oem_text(caption, x0 + pad, y0 + 3.0);
        draw_rectangle(x0, y0 + CELL_H + 5.0, w, 1.0, BLACK);
        let mut y = y0 + CELL_H + 6.0 + pad;
        for line in &lines {
            self.draw_oem_text(line, x0 + pad, y);
            y += CELL_H;
        }
        let (bx, by) = ((x0 + (w - btn_w) / 2.0).floor(), y + pad);
        frame(bx, by, btn_w, btn_h);
        self.draw_oem_text("OK", bx + (btn_w - 2.0 * CELL_W) / 2.0, by + 4.0);
        (bx, by, btn_w, btn_h)
    }

    /// The status display: a white box with a one-pixel black frame
    /// (FrameRect, stock black brush) at the client's top-right corner,
    /// labels at (2, 2 + 12 row), values at (label column + 2, ...).
    /// Returns the box's bottom edge.
    pub fn draw_status(&self, values: &[String; 4], client_w: f32) -> f32 {
        let label_w = LABELS.iter().map(|l| l.len()).max().unwrap_or(0) as f32 * CELL_W;
        let w = label_w + VALUE_TEMPLATE_CHARS * CELL_W + 4.0;
        let h = LABELS.len() as f32 * CELL_H + 4.0;
        let x0 = client_w - w;
        draw_rectangle(x0, 0.0, w, h, WHITE);
        draw_rectangle(x0, 0.0, w, 1.0, BLACK);
        draw_rectangle(x0, h - 1.0, w, 1.0, BLACK);
        draw_rectangle(x0, 0.0, 1.0, h, BLACK);
        draw_rectangle(x0 + w - 1.0, 0.0, 1.0, h, BLACK);
        for (row, (label, value)) in LABELS.iter().zip(values).enumerate() {
            let y = 2.0 + row as f32 * CELL_H;
            self.draw_oem_text(label, x0 + 2.0, y);
            self.draw_oem_text(value, x0 + 2.0 + label_w, y);
        }
        h
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
