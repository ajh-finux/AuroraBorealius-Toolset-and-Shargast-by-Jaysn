use std::collections::HashMap;
use eframe::egui;
use egui::load::SizedTexture;

const ICON_SIZE: u32 = 24;

pub struct Icons {
    textures: HashMap<String, egui::TextureHandle>,
}

impl Icons {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut textures = HashMap::new();
        let icon_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("icons");
        if !icon_dir.exists() {
            return Self { textures };
        }
        let entries = match std::fs::read_dir(&icon_dir) {
            Ok(e) => e,
            Err(_) => return Self { textures },
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("png") {
                continue;
            }
            let name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let img = match image::load_from_memory(&bytes) {
                Ok(i) => i,
                Err(_) => continue,
            };
            let gray = img.to_luma8();
            let img = image::imageops::resize(
                &gray, ICON_SIZE, ICON_SIZE,
                image::imageops::FilterType::Lanczos3,
            );
            let (w, h) = img.dimensions();
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let a = img.get_pixel(x, y)[0];
                    rgba.push(255);
                    rgba.push(255);
                    rgba.push(255);
                    rgba.push(a);
                }
            }
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                &rgba,
            );
            let handle = ctx.load_texture(
                &format!("icon_{}", name),
                color_image,
                egui::TextureOptions::LINEAR,
            );
            textures.insert(name, handle);
        }
        Self { textures }
    }

    pub fn get(&self, name: &str) -> Option<&egui::TextureHandle> {
        self.textures.get(name)
    }

    pub fn icon_button(&self, ui: &mut egui::Ui, name: &str, tooltip: &str, size: f32) -> egui::Response {
        if let Some(tex) = self.get(name) {
            let sized = SizedTexture::new(tex.id(), egui::vec2(size, size));
            let img = egui::Image::from_texture(sized);
            let btn = egui::ImageButton::new(img)
                .frame(false)
                .selected(false);
            ui.add(btn).on_hover_text(tooltip)
        } else {
            ui.button(tooltip)
        }
    }

    #[allow(dead_code)]
    pub fn icon_image(&self, ui: &mut egui::Ui, name: &str, size: f32) -> egui::Response {
        if let Some(tex) = self.get(name) {
            let sized = SizedTexture::new(tex.id(), egui::vec2(size, size));
            ui.image(sized)
        } else {
            ui.label("?")
        }
    }
}
