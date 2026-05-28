use std::collections::HashMap;
use eframe::egui::{self, ColorImage, TextureHandle};
use nwn_gamemodel::Module;

#[derive(Default)]
pub struct TilesetBrowserState {
    pub open: bool,
    selected_tileset: usize,
    selected_tile: Option<u32>,
    tileset_data: Vec<(String, Vec<(u32, String)>)>,
    texture_handles: HashMap<String, Vec<TextureHandle>>,
    loaded: bool,
}

impl TilesetBrowserState {
    pub fn reload(&mut self, module: &Module) {
        self.tileset_data.clear();
        self.texture_handles.clear();
        self.selected_tileset = 0;
        self.selected_tile = None;
        self.loaded = true;

        let area_names = module.area_names();
        let mut seen = std::collections::HashSet::new();
        let mut names = Vec::new();
        for aname in area_names {
            if let Some(area) = module.get_area(aname) {
                if seen.insert(area.tileset.clone()) {
                    names.push(area.tileset.clone());
                }
            }
        }
        names.sort();

        for name in &names {
            if let Some(ts) = module.tileset_info(name) {
                let mut tiles: Vec<(u32, String)> = ts.tiles.iter()
                    .map(|(id, def)| (*id, def.model_name.clone()))
                    .collect();
                tiles.sort_by_key(|(id, _)| *id);
                self.tileset_data.push((name.clone(), tiles));
            }
        }
    }

    fn load_thumbnails(&mut self, ctx: &egui::Context, model_name: &str) {
        if self.texture_handles.contains_key(model_name) {
            return;
        }
        let mut images = Vec::new();
        if let Ok(Some(meshes)) = nwn_tileset::load_model_meshes(model_name) {
            for mesh in &meshes {
                if let Some(ref tex_name) = mesh.texture_name {
                    if let Ok(Some(data)) = nwn_tileset::load_texture_data(tex_name) {
                        let img = ColorImage::from_rgba_unmultiplied(
                            [data.width as usize, data.height as usize],
                            &data.rgba,
                        );
                        images.push(img);
                    }
                }
            }
        }
        if images.is_empty() {
            let size = 64;
            let mut rgba = vec![0u8; size * size * 4];
            for y in 0..size {
                for x in 0..size {
                    let idx = (y * size + x) * 4;
                    rgba[idx] = 64;
                    rgba[idx + 1] = 64;
                    rgba[idx + 2] = 64;
                    rgba[idx + 3] = 255;
                    if (x / 8 + y / 8) % 2 == 0 {
                        rgba[idx] = 48;
                        rgba[idx + 1] = 48;
                        rgba[idx + 2] = 48;
                    }
                }
            }
            images.push(ColorImage::from_rgba_unmultiplied([size, size], &rgba));
        }
        let handles: Vec<TextureHandle> = images.into_iter().enumerate().map(|(i, img)| {
            ctx.load_texture(&format!("{}_{}", model_name, i), img, egui::TextureOptions::default())
        }).collect();
        self.texture_handles.insert(model_name.to_string(), handles);
    }
}

pub fn show_tileset_browser(state: &mut TilesetBrowserState, ctx: &egui::Context, module: Option<&Module>) {
    let mut open = true;
    let mut needs_reload = false;

    egui::Window::new("Visual Tileset Browser")
        .default_size([800.0, 500.0])
        .open(&mut open)
        .show(ctx, |ui| {
            if let Some(module) = module {
                if !state.loaded {
                    state.reload(module);
                }

                if state.tileset_data.is_empty() {
                    ui.label("No tilesets loaded. Open a module with areas first.");
                    return;
                }

                ui.horizontal(|ui| {
                    ui.label("Tileset:");
                    egui::ComboBox::from_id_salt("ts_selector")
                        .width(200.0)
                        .selected_text(&state.tileset_data[state.selected_tileset].0)
                        .show_ui(ui, |ui| {
                            for (i, (name, _)) in state.tileset_data.iter().enumerate() {
                                ui.selectable_value(&mut state.selected_tileset, i, name);
                            }
                        });
                    if ui.button("Reload").clicked() {
                        needs_reload = true;
                    }
                });

                ui.separator();

                let (ref ts_name, ref tiles) = state.tileset_data[state.selected_tileset].clone();
                ui.label(format!("{} — {} tile(s)", ts_name, tiles.len()));
                ui.separator();

                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("tile_grid")
                            .min_col_width(140.0)
                            .max_col_width(180.0)
                            .spacing([8.0, 8.0])
                            .show(ui, |ui| {
                                let mut tile_count = 0;
                                for &(tile_id, ref model_name) in tiles {
                                    if tile_count > 0 && tile_count % 4 == 0 {
                                        ui.end_row();
                                    }
                                    tile_count += 1;

                                    state.load_thumbnails(ctx, model_name);

                                    let selected = state.selected_tile == Some(tile_id);
                                    let bg = if selected { ui.style().visuals.selection.bg_fill } else { ui.style().visuals.window_fill() };
                                    let stroke = if selected { ui.style().visuals.selection.stroke } else { ui.style().visuals.window_stroke() };

                                    egui::Frame::new()
                                        .fill(bg)
                                        .stroke(stroke)
                                        .inner_margin(egui::Margin::same(4))
                                        .show(ui, |ui| {
                                            ui.set_min_size(egui::vec2(128.0, 128.0));
                                            ui.vertical(|ui| {
                                                if let Some(handles) = state.texture_handles.get(model_name) {
                                                    for handle in handles {
                                                        ui.add(egui::Image::new(handle).max_width(120.0).max_height(100.0));
                                                    }
                                                }

                                                let label = format!("ID {} — {}", tile_id, model_name);
                                                if ui.selectable_label(selected, &label).clicked() {
                                                    state.selected_tile = Some(tile_id);
                                                }
                                            });
                                        });
                                }
                            });
                    });
            } else {
                state.loaded = false;
                ui.label("No module loaded.");
            }
        });

    if needs_reload {
        if let Some(module) = module {
            state.reload(module);
        }
    }

    if !open {
        state.open = false;
    }
}
