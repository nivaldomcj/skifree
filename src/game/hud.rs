use macroquad::prelude::*;

pub struct Hud {
    pub time: String,
    pub dist: String,
    pub speed: String,
    pub style: String,
    font_size: u16,
}

impl Hud {
    pub fn new() -> Self {
        Self {
            time: "00:00:00.00".to_string(),
            dist: " 0000m".to_string(),
            speed: " 0000m/s".to_string(),
            style: "0000000".to_string(),
            font_size: 14,
        }
    }

    pub fn draw(&self, y_offset: f32) {
        let labels = ["Time:", "Dist:", "Speed:", "Style:"];
        let values = [&self.time, &self.dist, &self.speed, &self.style];
        let spacing = screen_width() / 4.0;

        for (i, (label, value)) in labels.iter().zip(values.iter()).enumerate() {
            let x = i as f32 * spacing + 4.0;
            draw_text(label, x, y_offset + 14.0, self.font_size as f32, DARKGRAY);
            let label_width = measure_text(label, None, self.font_size, 1.0).width;
            draw_text(
                value,
                x + label_width + 2.0,
                y_offset + 14.0,
                self.font_size as f32,
                BLACK,
            );
        }
    }
}
