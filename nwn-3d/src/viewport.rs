use std::path::Path;
use std::sync::{Arc, Mutex};

use egui_wgpu::{CallbackTrait, CallbackResources, ScreenDescriptor};
use egui::epaint::PaintCallbackInfo;
use wgpu::util::DeviceExt;

use crate::camera::{OrbitCamera, mat4_mul};
use crate::pipeline;
use crate::tile_renderer::{self, MarkerInfo};

struct TextureSlot {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
    pending: Option<(u32, u32, Vec<u8>)>,
}

struct GroupResources {
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    num_indices: u32,
    instance_buf: wgpu::Buffer,
    num_instances: u32,
    texture_slot: usize,
}

pub struct ViewportState {
    pub camera: OrbitCamera,
    pub area_version: u64,
    pub markers: Vec<MarkerInfo>,
    pub tile_count: usize,
    color_format: wgpu::TextureFormat,
    marker_idx_count: u32,
    pub draw_calls: usize,
    pub group_count: usize,
    marker_vertex_buf: Option<wgpu::Buffer>,
    marker_index_buf: Option<wgpu::Buffer>,

    walkmesh_idx_count: u32,
    walkmesh_vertex_buf: Option<wgpu::Buffer>,
    walkmesh_index_buf: Option<wgpu::Buffer>,
    show_walkmesh: bool,
    walkmesh_edge_vbuf: Option<wgpu::Buffer>,
    walkmesh_edge_ibuf: Option<wgpu::Buffer>,
    walkmesh_edge_count: u32,
    show_walkmesh_edges: bool,
    paint_idx_count: u32,
    paint_vertex_buf: Option<wgpu::Buffer>,
    paint_index_buf: Option<wgpu::Buffer>,
    show_paint: bool,
    pub clear_color: wgpu::Color,
    pub water_level: i32,
    pub show_water: bool,
    pub water_color: [u8; 4],
    show_tiles: bool,
    uniform_buf: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    groups: Vec<GroupResources>,
    texture_slots: Vec<TextureSlot>,
    default_instance_buf: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    marker_pipeline: wgpu::RenderPipeline,
    walkmesh_pipeline: wgpu::RenderPipeline,
    viewport_size: (u32, u32),
    color_tex: [Option<wgpu::Texture>; 2],
    color_view: [Option<wgpu::TextureView>; 2],
    depth_tex: [Option<wgpu::Texture>; 2],
    depth_view: [Option<wgpu::TextureView>; 2],
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_bind_group: [Option<wgpu::BindGroup>; 2],
    blit_sampler: wgpu::Sampler,
    frame_index: usize,

    highlight_vbuf: Option<wgpu::Buffer>,
    highlight_ibuf: Option<wgpu::Buffer>,
    highlight_count: u32,
    highlight_pipeline: wgpu::RenderPipeline,

    floor_pipeline: wgpu::RenderPipeline,
    floor_vertex_buf: Option<wgpu::Buffer>,
    floor_index_buf: Option<wgpu::Buffer>,
    floor_idx_count: u32,
    show_grid: bool,
    lighting_enabled: bool,
    show_fog: bool,
    fog_color: [f32; 4],
    fog_amount_norm: f32,
    fog_clip_dist: f32,
    ambient_color: [f32; 3],
    diffuse_color: [f32; 3],
}

fn load_texture(name: &str, module_erf: Option<&nwn_erf::ErfFile>, hak_index: Option<&nwn_tileset::HakIndex>, mod_path: Option<&Path>, _hak_names: &[String]) -> Option<(u32, u32, Vec<u8>)> {
    // Try module ERF first
    if let Some(erf) = module_erf {
        if let Some(data) = nwn_tileset::load_texture_data_from_sources(name, &[erf]).ok().flatten() {
            return Some((data.width, data.height, data.rgba));
        }
    }
    // Try specific hak via HakIndex (fast, no full file reads)
    if let (Some(idx), Some(dir)) = (hak_index, mod_path) {
        for &restype in &[8u16, 3u16, 1u16] {
            if let Some(data) = idx.get_resource_from_path(name, restype, dir) {
                if let Some(tex) = nwn_tileset::decode_texture_data(restype, &data) {
                    return Some((tex.width, tex.height, tex.rgba));
                }
            }
        }
    }
    // Fall back to game data
    match nwn_tileset::load_texture_data(name) {
        Ok(Some(data)) => {
            Some((data.width, data.height, data.rgba))
        }
        Ok(None) => {
            None
        }
        Err(e) => {
            eprintln!("ERROR: Failed to load texture '{name}': {e}");
            None
        }
    }
}

impl ViewportState {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        area: &nwn_gamemodel::Area,
        area_version: u64,
        visibility_filter: tile_renderer::VisibilityFilter,
        existing_camera: Option<OrbitCamera>,
        module_erf: Option<&nwn_erf::ErfFile>,
        mod_path: Option<&std::path::Path>,
        hak_names: &[String],
    ) -> Self {

        let camera = existing_camera.unwrap_or_else(|| OrbitCamera::new(area.width as f32, area.height as f32));
        let show_tiles = visibility_filter.show_tiles;
        let show_walkmesh = false;

        // Build HakIndex for efficient texture search (only opens the relevant hak, not all haks)
        let hak_index = mod_path.and_then(|p| {
            if !hak_names.is_empty() {
                Some(nwn_tileset::HakIndex::new(p, hak_names))
            } else {
                None
            }
        });

        // Collect unique model names from area for parallel loading
        let unique_models: Vec<String> = {
            let cols_p = area.width.max(1) as usize;
            let rows_p = area.height.max(1) as usize;
            let mut seen = std::collections::HashSet::new();
            let mut names = Vec::new();
            for (i, _) in area.tiles.iter().enumerate() {
                if i >= cols_p * rows_p { break; }
                if let Some(Some(name)) = area.tile_models.get(i) {
                    if !name.is_empty() && seen.insert(name.clone()) {
                        names.push(name.clone());
                    }
                }
            }
            // Also collect object model names
            for p in &area.placeables {
                if let Some(ref name) = p.model_name {
                    if !name.is_empty() && seen.insert(name.clone()) {
                        names.push(name.clone());
                    }
                }
            }
            for d in &area.doors {
                if let Some(ref name) = d.model_name {
                    if !name.is_empty() && seen.insert(name.clone()) {
                        names.push(name.clone());
                    }
                }
            }
            for c in &area.creatures {
                if let Some(ref name) = c.model_name {
                    if !name.is_empty() && seen.insert(name.clone()) {
                        names.push(name.clone());
                    }
                }
            }
            names
        };

        // Load all models in parallel
        let mut model_cache = tile_renderer::ModelCache::new();
        {
            use rayon::prelude::*;
            let results: Vec<_> = unique_models.par_iter()
                .map(|name| {
                    // Try game data first
                    let mut loaded = nwn_tileset::load_model_meshes(name).ok().flatten();
                    // Fall back to module ERF
                    if loaded.is_none() {
                        if let Some(erf) = module_erf {
                            // Try MDL (2002) then MDB (2001)
                            for restype in &[2002u16, 2001] {
                                if let Some(data) = erf.get(name, *restype) {
                                    if let Ok(meshes) = nwn_tileset::load_model_meshes_from_data(name, &data) {
                                        loaded = meshes;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // Fall back to haks (one at a time to avoid OOM)
                    if loaded.is_none() {
                        if let (Some(dir), hak_list) = (mod_path, hak_names) {
                            if !hak_list.is_empty() {
                                loaded = nwn_tileset::load_model_meshes_from_haks(name, dir, hak_list);
                            }
                        }
                    }
                    if loaded.is_some() {
                        eprintln!("  DEBUG model '{}' loaded OK", name);
                    } else {
                        eprintln!("  DEBUG model '{}' NOT FOUND", name);
                    }
                    (name.clone(), loaded)
                })
                .collect();
            for (name, meshes) in results {
                if let Some(m) = meshes {
                    model_cache.cache.insert(name, m);
                }
            }
        }

        let mut instanced = tile_renderer::build_instanced_tiles(area, &mut model_cache);
        let obj_instanced = tile_renderer::build_object_models(area, &mut model_cache);
        // Merge object models into instanced
        instanced.models.extend(obj_instanced.models);
        instanced.instance_data.extend(obj_instanced.instance_data);

        let markers_result = tile_renderer::build_object_markers(area, visibility_filter);
        let marker_data = markers_result.mesh;
        let tile_count: usize = instanced.instance_data.iter().map(|v| v.len()).sum();
        let draw_calls = instanced.instance_data.iter().filter(|v| !v.is_empty()).count() + 1;
        let group_count = instanced.instance_data.len();

        // Collect unique texture names from models
        let mut tex_names: Vec<String> = Vec::new();
        for m in &instanced.models {
            if let Some(ref tn) = m.texture_name {
                if !tex_names.iter().any(|n| n == tn) {
                    tex_names.push(tn.clone());
                }
            }
        }

        // Load all textures sequentially (hak fallback can't handle parallel opens of multi-GB files)
        let tex_data: Vec<(String, Option<(u32, u32, Vec<u8>)>)> = {
            tex_names.iter()
                .map(|name| (name.clone(), load_texture(name, module_erf, hak_index.as_ref(), mod_path, hak_names)))
                .collect()
        };

        // Create texture slots from loaded data
        let mut texture_slots: Vec<TextureSlot> = tex_data.iter().map(|(name, pending)| {
            let (w, h) = pending.as_ref().map(|d| (d.0, d.1)).unwrap_or((2, 2));
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("nwn3d_tex_{name}")),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            TextureSlot {
                view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
                sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("nwn3d_tex_sampler"),
                    address_mode_u: wgpu::AddressMode::Repeat,
                    address_mode_v: wgpu::AddressMode::Repeat,
                    address_mode_w: wgpu::AddressMode::Repeat,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                }),
                texture,
                bind_group: None,
                pending: pending.clone(),
            }
        }).collect();

        // Map texture name -> slot index (+1 for fallback at 0)
        let tex_slot_map: std::collections::HashMap<&str, usize> = tex_names.iter()
            .enumerate().map(|(i, n)| (n.as_str(), i + 1)).collect();

        // Create per-model group resources with instance buffers
        let mut group_resources = Vec::new();
        for (midx, model) in instanced.models.iter().enumerate() {
            let tex_slot = model.texture_name.as_ref()
                .and_then(|n| tex_slot_map.get(n.as_str()).copied())
                .unwrap_or(0);

            let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_inst_vbuf"),
                contents: bytemuck::cast_slice(&model.data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_inst_ibuf"),
                contents: bytemuck::cast_slice(&model.data.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let instance_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_inst_instance_buf"),
                contents: bytemuck::cast_slice(&instanced.instance_data[midx]),
                usage: wgpu::BufferUsages::VERTEX,
            });

            group_resources.push(GroupResources {
                num_indices: model.data.indices.len() as u32,
                num_instances: instanced.instance_data[midx].len() as u32,
                vertex_buf,
                index_buf,
                instance_buf,
                texture_slot: tex_slot,
            });
        }

        // Add a fallback texture slot at index 0 (2x2 white)
        let fb_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nwn3d_fallback_tex"),
            size: wgpu::Extent3d { width: 2, height: 2, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let fb_view = fb_tex.create_view(&wgpu::TextureViewDescriptor::default());
        texture_slots.insert(0, TextureSlot {
            texture: fb_tex,
            view: fb_view,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("nwn3d_fallback_sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            bind_group: None,
            pending: None,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("nwn3d_uniform_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("nwn3d_texture_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline = pipeline::create_pipeline(device, config, &uniform_bind_group_layout, &texture_bind_group_layout, Some(wgpu::TextureFormat::Depth32Float));
        let marker_pipeline = pipeline::create_marker_pipeline(
            device,
            config.format,
            &uniform_bind_group_layout,
            &texture_bind_group_layout,
            Some(wgpu::TextureFormat::Depth32Float),
        );
        let walkmesh_pipeline = pipeline::create_walkmesh_pipeline(
            device,
            config.format,
            &uniform_bind_group_layout,
            &texture_bind_group_layout,
            Some(wgpu::TextureFormat::Depth32Float),
        );
        let floor_pipeline = pipeline::create_floor_pipeline(
            device,
            config.format,
            &uniform_bind_group_layout,
            &texture_bind_group_layout,
            Some(wgpu::TextureFormat::Depth32Float),
        );

        let walkmesh_data = tile_renderer::build_walkmesh(area);
        let walkmesh_vertex_buf = if !walkmesh_data.vertices.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_walkmesh_vbuf"),
                contents: bytemuck::cast_slice(&walkmesh_data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else { None };
        let walkmesh_index_buf = if !walkmesh_data.indices.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_walkmesh_ibuf"),
                contents: bytemuck::cast_slice(&walkmesh_data.indices),
                usage: wgpu::BufferUsages::INDEX,
            }))
        } else { None };

        let (walkmesh_edge_verts, walkmesh_edge_idx) = tile_renderer::build_walkmesh_edges(area);
        let walkmesh_edge_vbuf = if !walkmesh_edge_verts.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_walkedge_vbuf"),
                contents: bytemuck::cast_slice(&walkmesh_edge_verts),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else { None };
        let walkmesh_edge_ibuf = if !walkmesh_edge_idx.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_walkedge_ibuf"),
                contents: bytemuck::cast_slice(&walkmesh_edge_idx),
                usage: wgpu::BufferUsages::INDEX,
            }))
        } else { None };
        let walkmesh_edge_count = walkmesh_edge_idx.len() as u32;

        let floor_data = tile_renderer::build_area_floor(area);
        let floor_vertex_buf = if !floor_data.vertices.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_floor_vbuf"),
                contents: bytemuck::cast_slice(&floor_data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else { None };
        let floor_index_buf = if !floor_data.indices.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_floor_ibuf"),
                contents: bytemuck::cast_slice(&floor_data.indices),
                usage: wgpu::BufferUsages::INDEX,
            }))
        } else { None };
        let floor_idx_count = floor_data.indices.len() as u32;

        let paint_data = tile_renderer::build_paint_overlay_ex(area, 0, false, [40, 80, 200, 160]);
        let paint_vertex_buf = if !paint_data.vertices.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_paint_vbuf"),
                contents: bytemuck::cast_slice(&paint_data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else { None };
        let paint_index_buf = if !paint_data.indices.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_paint_ibuf"),
                contents: bytemuck::cast_slice(&paint_data.indices),
                usage: wgpu::BufferUsages::INDEX,
            }))
        } else { None };

        let marker_vertex_buf = if !marker_data.vertices.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_marker_vbuf"),
                contents: bytemuck::cast_slice(&marker_data.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        } else { None };
        let marker_index_buf = if !marker_data.indices.is_empty() {
            Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_marker_ibuf"),
                contents: bytemuck::cast_slice(&marker_data.indices),
                usage: wgpu::BufferUsages::INDEX,
            }))
        } else { None };

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("nwn3d_uniform_buf"),
            size: std::mem::size_of::<[f32; 36]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nwn3d_uniform_bg"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        // Create blit pipeline (full-screen quad to display our offscreen render)
        let blit_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("nwn3d_blit_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nwn3d_blit_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let blit_pipeline = create_blit_pipeline(device, config, &blit_bind_group_layout);
        let highlight_pipeline = pipeline::create_highlight_pipeline(device, config.format, &uniform_bind_group_layout);

        let marker_idx_count = marker_data.indices.len() as u32;
        let walkmesh_idx_count = walkmesh_data.indices.len() as u32;
        let paint_idx_count = paint_data.indices.len() as u32;

        Self {
            camera,
            area_version,
            markers: markers_result.markers,
            tile_count,
            color_format: config.format,
            marker_idx_count,
            draw_calls,
            group_count,
            marker_vertex_buf,
            marker_index_buf,
            walkmesh_idx_count,
            walkmesh_vertex_buf,
            walkmesh_index_buf,
            show_walkmesh,
            walkmesh_edge_vbuf,
            walkmesh_edge_ibuf,
            walkmesh_edge_count,
            show_walkmesh_edges: false,
            clear_color: wgpu::Color { r: 0.1, g: 0.1, b: 0.12, a: 1.0 },
            water_level: 0,
            show_water: false,
            water_color: [40, 80, 200, 160],
            paint_idx_count,
            paint_vertex_buf,
            paint_index_buf,
            show_paint: false,
            show_tiles,
            uniform_buf,
            uniform_bind_group,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            groups: group_resources,
            texture_slots,
            default_instance_buf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("nwn3d_default_instance"),
                contents: bytemuck::cast_slice(&[tile_renderer::InstanceData {
                    row0: [1.0, 0.0, 0.0, 0.0],
                    row1: [0.0, 1.0, 0.0, 0.0],
                    row2: [0.0, 0.0, 1.0, 0.0],
                    row3: [0.0, 0.0, 0.0, 1.0],
                }]),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            pipeline,
            marker_pipeline,
            walkmesh_pipeline,
            viewport_size: (0, 0),
            color_tex: [None, None],
            color_view: [None, None],
            depth_tex: [None, None],
            depth_view: [None, None],
            blit_pipeline,
            blit_bind_group_layout,
            blit_bind_group: [None, None],
            blit_sampler,
            frame_index: 0,

            highlight_vbuf: None,
            highlight_ibuf: None,
            highlight_count: 0,
            highlight_pipeline,

            floor_pipeline,
            floor_vertex_buf,
            floor_index_buf,
            floor_idx_count,
            show_grid: true,
            lighting_enabled: true,
            show_fog: false,
            fog_color: [
                area.sun_fog_color[0] as f32 / 255.0,
                area.sun_fog_color[1] as f32 / 255.0,
                area.sun_fog_color[2] as f32 / 255.0,
                1.0,
            ],
            fog_amount_norm: area.sun_fog_amount as f32 / 255.0,
            fog_clip_dist: area.fog_clip_dist,
            ambient_color: [
                area.sun_ambient_color[0] as f32 / 255.0,
                area.sun_ambient_color[1] as f32 / 255.0,
                area.sun_ambient_color[2] as f32 / 255.0,
            ],
            diffuse_color: [
                area.sun_diffuse_color[0] as f32 / 255.0,
                area.sun_diffuse_color[1] as f32 / 255.0,
                area.sun_diffuse_color[2] as f32 / 255.0,
            ],
        }
    }

    fn ensure_scene_textures(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.viewport_size.0 == width && self.viewport_size.1 == height && self.color_tex[0].is_some() {
            return;
        }
        self.viewport_size = (width, height);

        let color_desc = wgpu::TextureDescriptor {
            label: Some("nwn3d_scene_color"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let depth_desc = wgpu::TextureDescriptor {
            label: Some("nwn3d_scene_depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        for i in 0..2 {
            let ct = device.create_texture(&color_desc);
            let cv = ct.create_view(&wgpu::TextureViewDescriptor::default());
            let dt = device.create_texture(&depth_desc);
            let dv = dt.create_view(&wgpu::TextureViewDescriptor::default());
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("nwn3d_blit_bg_{i}")),
                layout: &self.blit_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&cv),
                    },
                ],
            });
            self.color_tex[i] = Some(ct);
            self.color_view[i] = Some(cv);
            self.depth_tex[i] = Some(dt);
            self.depth_view[i] = Some(dv);
            self.blit_bind_group[i] = Some(bg);
        }

        // Recreate all pipelines with the offscreen color format
        self.pipeline = pipeline::create_pipeline_with_depth(
            device,
            self.color_format,
            &self.uniform_bind_group_layout,
            &self.texture_bind_group_layout,
            Some(wgpu::TextureFormat::Depth32Float),
        );
        self.marker_pipeline = pipeline::create_marker_pipeline(
            device,
            self.color_format,
            &self.uniform_bind_group_layout,
            &self.texture_bind_group_layout,
            Some(wgpu::TextureFormat::Depth32Float),
        );
        self.walkmesh_pipeline = pipeline::create_walkmesh_pipeline(
            device,
            self.color_format,
            &self.uniform_bind_group_layout,
            &self.texture_bind_group_layout,
            Some(wgpu::TextureFormat::Depth32Float),
        );
        self.highlight_pipeline = pipeline::create_highlight_pipeline(
            device,
            self.color_format,
            &self.uniform_bind_group_layout,
        );
        self.floor_pipeline = pipeline::create_floor_pipeline(
            device,
            self.color_format,
            &self.uniform_bind_group_layout,
            &self.texture_bind_group_layout,
            Some(wgpu::TextureFormat::Depth32Float),
        );
    }

    pub fn update_uniforms(&mut self, queue: &wgpu::Queue, viewport_size: (u32, u32)) {
        self.camera.aspect = if viewport_size.1 > 0 {
            viewport_size.0 as f32 / viewport_size.1 as f32
        } else {
            1.0
        };
        let vp = mat4_mul(self.camera.view_matrix(), self.camera.projection_matrix());
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::cast_slice(&[vp]));
        // ambient_color at offset 64 (float[16]): xyz = ambient RGB, w = lighting flag
        let ambient = [
            self.ambient_color[0],
            self.ambient_color[1],
            self.ambient_color[2],
            if self.lighting_enabled { 1.0f32 } else { 0.0f32 },
        ];
        queue.write_buffer(&self.uniform_buf, 64, bytemuck::cast_slice(&ambient));
        // fog_color at offset 80 (float[20])
        queue.write_buffer(&self.uniform_buf, 80, bytemuck::cast_slice(&self.fog_color));
        // fog_params at offset 96 (float[24]): x=fog_amount_norm, y=fog_clip_dist
        let fog_params = [
            if self.show_fog { self.fog_amount_norm } else { 0.0 },
            self.fog_clip_dist,
            0.0, 0.0,
        ];
        queue.write_buffer(&self.uniform_buf, 96, bytemuck::cast_slice(&fog_params));
        // camera_pos at offset 112 (float[28])
        let eye = self.camera.eye();
        queue.write_buffer(&self.uniform_buf, 112, bytemuck::cast_slice(&[eye[0], eye[1], eye[2], 0.0]));
        // diffuse_color at offset 128 (float[32])
        queue.write_buffer(&self.uniform_buf, 128, bytemuck::cast_slice(&self.diffuse_color));
    }

    pub fn render_3d(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        let fb_bg = self.texture_slots[0].bind_group.as_ref();

        // Render floor grid overlay (below tiles, always-visible depth)
        if self.show_grid {
            if let (Some(vbuf), Some(ibuf)) = (&self.floor_vertex_buf, &self.floor_index_buf) {
                if self.floor_idx_count > 0 {
                    render_pass.set_pipeline(&self.floor_pipeline);
                    render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                    if let Some(bg) = fb_bg {
                        render_pass.set_bind_group(1, bg, &[]);
                    }
                    render_pass.set_vertex_buffer(0, vbuf.slice(..));
                    render_pass.set_vertex_buffer(1, self.default_instance_buf.slice(..));
                    render_pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.floor_idx_count, 0, 0..1);
                }
            }
        }

        // Render instanced tile groups (depth-tested)
        if self.show_tiles {
            let use_fallback = std::env::var("NWN_NO_TEX").is_ok();
            for grp in &self.groups {
                let bg = if use_fallback {
                    fb_bg
                } else {
                    let tex_slot = &self.texture_slots[grp.texture_slot];
                    tex_slot.bind_group.as_ref()
                };
                let bg = match bg {
                    Some(b) => b,
                    None => continue,
                };
                render_pass.set_bind_group(1, bg, &[]);
                render_pass.set_vertex_buffer(0, grp.vertex_buf.slice(..));
                render_pass.set_vertex_buffer(1, grp.instance_buf.slice(..));
                render_pass.set_index_buffer(grp.index_buf.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..grp.num_indices, 0, 0..grp.num_instances);
            }
        }

        // Render object markers (non-instanced, single batch) — use fallback texture, LessEqual depth
        if let (Some(bg), Some(mvbuf), Some(mibuf)) = (fb_bg, &self.marker_vertex_buf, &self.marker_index_buf) {
            if self.marker_idx_count > 0 {
                render_pass.set_pipeline(&self.marker_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_bind_group(1, bg, &[]);
                render_pass.set_vertex_buffer(0, mvbuf.slice(..));
                render_pass.set_vertex_buffer(1, self.default_instance_buf.slice(..));
                render_pass.set_index_buffer(mibuf.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.marker_idx_count, 0, 0..1);
            }
        }

        // Render walkmesh overlay (tile height-based semi-transparent green, depth-tested)
        if self.show_walkmesh {
            if let (Some(bg), Some(vbuf), Some(ibuf)) = (fb_bg, &self.walkmesh_vertex_buf, &self.walkmesh_index_buf) {
                if self.walkmesh_idx_count > 0 {
                    render_pass.set_pipeline(&self.walkmesh_pipeline);
                    render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                    render_pass.set_bind_group(1, bg, &[]);
                    render_pass.set_vertex_buffer(0, vbuf.slice(..));
                    render_pass.set_vertex_buffer(1, self.default_instance_buf.slice(..));
                    render_pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.walkmesh_idx_count, 0, 0..1);
                }
            }
        }

        // Render walkmesh edge wireframe (line overlay, uses highlight/LINE pipeline)
        if self.show_walkmesh_edges {
            if let (Some(vbuf), Some(ibuf)) = (&self.walkmesh_edge_vbuf, &self.walkmesh_edge_ibuf) {
                if self.walkmesh_edge_count > 0 {
                    render_pass.set_pipeline(&self.highlight_pipeline);
                    render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                    render_pass.set_vertex_buffer(0, vbuf.slice(..));
                    render_pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.walkmesh_edge_count, 0, 0..1);
                }
            }
        }

        // Render paint overlay (colored quads on tiles with user tints)
        if self.show_paint {
            if let (Some(bg), Some(vbuf), Some(ibuf)) = (fb_bg, &self.paint_vertex_buf, &self.paint_index_buf) {
                if self.paint_idx_count > 0 {
                    render_pass.set_pipeline(&self.walkmesh_pipeline);
                    render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                    render_pass.set_bind_group(1, bg, &[]);
                    render_pass.set_vertex_buffer(0, vbuf.slice(..));
                    render_pass.set_vertex_buffer(1, self.default_instance_buf.slice(..));
                    render_pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.paint_idx_count, 0, 0..1);
                }
            }
        }

        // Render tile highlight wireframe
        if let (Some(vbuf), Some(ibuf)) = (&self.highlight_vbuf, &self.highlight_ibuf) {
            if self.highlight_count > 0 {
                render_pass.set_pipeline(&self.highlight_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vbuf.slice(..));
                render_pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.highlight_count, 0, 0..1);
            }
        }
    }



    pub fn raypick(&self, screen_x: f32, screen_y: f32, view_w: f32, view_h: f32) -> Option<(usize, &MarkerInfo)> {
        if view_w <= 0.0 || view_h <= 0.0 { return None; }
        let (origin, dir) = self.camera.ray_from_screen(screen_x, screen_y, view_w, view_h);
        let half = 0.5;
        let eps = 1e-15;
        let inv_dir = [
            if dir[0].abs() > eps { 1.0 / dir[0] } else { f32::INFINITY },
            if dir[1].abs() > eps { 1.0 / dir[1] } else { f32::INFINITY },
            if dir[2].abs() > eps { 1.0 / dir[2] } else { f32::INFINITY },
        ];
        for (idx, marker) in self.markers.iter().enumerate() {
            let pos_x = marker.x;
            let pos_y = marker.z;
            let pos_z = -marker.y;
            let tmin = ((pos_x - half - origin[0]) * inv_dir[0])
                .min((pos_x + half - origin[0]) * inv_dir[0]);
            let tmax = ((pos_x - half - origin[0]) * inv_dir[0])
                .max((pos_x + half - origin[0]) * inv_dir[0]);
            let tymin = ((pos_y - half - origin[1]) * inv_dir[1])
                .min((pos_y + half - origin[1]) * inv_dir[1]);
            let tymax = ((pos_y - half - origin[1]) * inv_dir[1])
                .max((pos_y + half - origin[1]) * inv_dir[1]);
            let tzmin = ((pos_z - half - origin[2]) * inv_dir[2])
                .min((pos_z + half - origin[2]) * inv_dir[2]);
            let tzmax = ((pos_z - half - origin[2]) * inv_dir[2])
                .max((pos_z + half - origin[2]) * inv_dir[2]);
            let t0 = tmin.max(tymin).max(tzmin);
            let t1 = tmax.min(tymax).min(tzmax);
            if t0 <= t1 && t1 >= 0.0 {
                return Some((idx, marker));
            }
        }
        None
    }

    /// Returns (world_x, world_y, world_z) where the ray hits the ground plane (Y=0 in renderer space).
    pub fn pick_ground(&self, screen_x: f32, screen_y: f32, view_w: f32, view_h: f32) -> Option<[f32; 3]> {
        if view_w <= 0.0 || view_h <= 0.0 { return None; }
        let (origin, dir) = self.camera.ray_from_screen(screen_x, screen_y, view_w, view_h);
        if dir[1].abs() < 1e-15 { return None; }
        let t = -origin[1] / dir[1];
        if t < 0.0 { return None; }
        Some([origin[0] + t * dir[0], origin[1] + t * dir[1], origin[2] + t * dir[2]])
    }

    pub fn set_highlight_tile(&mut self, device: &wgpu::Device, tile_idx: Option<usize>, col: usize, row: usize, height: f32) {
        self.highlight_vbuf = None;
        self.highlight_ibuf = None;
        self.highlight_count = 0;

        if tile_idx.is_none() { return; }

        let tile_w = 10.0;
        let tx = col as f32 * tile_w;
        let tz = -(row as f32) * tile_w;
        let hy = height;
        let hh = 0.25;

        let corners: [[f32; 3]; 8] = [
            [tx,      hy - hh, tz],
            [tx + tile_w, hy - hh, tz],
            [tx + tile_w, hy - hh, tz - tile_w],
            [tx,      hy - hh, tz - tile_w],
            [tx,      hy + hh, tz],
            [tx + tile_w, hy + hh, tz],
            [tx + tile_w, hy + hh, tz - tile_w],
            [tx,      hy + hh, tz - tile_w],
        ];

        let edges: [(u32, u32); 12] = [
            (0,1),(1,2),(2,3),(3,0),  // bottom
            (4,5),(5,6),(6,7),(7,4),  // top
            (0,4),(1,5),(2,6),(3,7),  // vertical
        ];

        let color: [f32; 4] = [0.0, 1.0, 1.0, 1.0]; // bright cyan
        let mut verts: Vec<f32> = Vec::new();
        for &(a, b) in &edges {
            for &ci in &[a, b] {
                verts.push(corners[ci as usize][0]);
                verts.push(corners[ci as usize][1]);
                verts.push(corners[ci as usize][2]);
                verts.push(color[0]);
                verts.push(color[1]);
                verts.push(color[2]);
                verts.push(color[3]);
            }
        }
        let indices: Vec<u32> = (0..(edges.len() * 2) as u32).collect();

        if verts.is_empty() { return; }

        self.highlight_vbuf = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("nwn3d_highlight_vbuf"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        }));
        self.highlight_ibuf = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("nwn3d_highlight_ibuf"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        }));
        self.highlight_count = indices.len() as u32;
    }

    fn ensure_textures(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        for slot in &mut self.texture_slots {
            if slot.bind_group.is_some() {
                continue;
            }
            if let Some((width, height, ref data)) = slot.pending.take() {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &slot.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 4),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                );
            } else {
                // Default 2x2 white for fallback/no-data
                let white: [u8; 16] = [255; 16];
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &slot.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &white,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(8),
                        rows_per_image: Some(2),
                    },
                    wgpu::Extent3d { width: 2, height: 2, depth_or_array_layers: 1 },
                );
            }
            slot.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("nwn3d_tex_bg"),
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&slot.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&slot.view),
                    },
                ],
            }));
        }
    }
}

struct PendingArea {
    area: Arc<nwn_gamemodel::Area>,
    version: u64,
    format: wgpu::TextureFormat,
}

pub struct WgpuViewport {
    pub inner: Arc<Mutex<Option<ViewportState>>>,
    pending: Arc<Mutex<Option<PendingArea>>>,
    visibility_filter: Arc<Mutex<tile_renderer::VisibilityFilter>>,
    module_erf_bytes: Arc<Mutex<Option<Arc<Vec<u8>>>>>,
    mod_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    hak_names: Arc<Mutex<Vec<String>>>,
}

impl WgpuViewport {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(None)),
            visibility_filter: Arc::new(Mutex::new(Default::default())),
            module_erf_bytes: Arc::new(Mutex::new(None)),
            mod_path: Arc::new(Mutex::new(None)),
            hak_names: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn clone_arc(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            pending: Arc::clone(&self.pending),
            visibility_filter: Arc::clone(&self.visibility_filter),
            module_erf_bytes: Arc::clone(&self.module_erf_bytes),
            mod_path: Arc::clone(&self.mod_path),
            hak_names: Arc::clone(&self.hak_names),
        }
    }

    pub fn set_module_erf(&self, data: Option<Vec<u8>>) {
        if let Ok(mut guard) = self.module_erf_bytes.lock() {
            *guard = data.map(Arc::new);
        }
    }

    pub fn set_hak_info(&self, path: Option<std::path::PathBuf>, haks: Vec<String>) {
        if let Ok(mut guard) = self.mod_path.lock() {
            *guard = path;
        }
        if let Ok(mut guard) = self.hak_names.lock() {
            *guard = haks;
        }
    }

    pub fn set_visibility_filter(&self, filter: tile_renderer::VisibilityFilter) {
        if let Ok(mut guard) = self.visibility_filter.lock() {
            *guard = filter;
        }
    }

    pub fn set_area(&self, area: Arc<nwn_gamemodel::Area>, version: u64, format: wgpu::TextureFormat) {
        if let Ok(mut guard) = self.pending.lock() {
            *guard = Some(PendingArea { area, version, format });
        }
    }

    pub fn set_show_grid(&self, show: bool) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.show_grid = show;
            }
        }
    }

    pub fn get_show_grid(&self) -> bool {
        if let Ok(guard) = self.inner.lock() {
            if let Some(ref state) = *guard {
                return state.show_grid;
            }
        }
        true
    }

    pub fn set_show_walkmesh_edges(&self, show: bool) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.show_walkmesh_edges = show;
            }
        }
    }

    pub fn get_show_walkmesh_edges(&self) -> bool {
        if let Ok(guard) = self.inner.lock() {
            if let Some(ref state) = *guard {
                return state.show_walkmesh_edges;
            }
        }
        false
    }

    pub fn set_lighting_enabled(&self, enabled: bool) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.lighting_enabled = enabled;
            }
        }
    }

    pub fn get_lighting_enabled(&self) -> bool {
        if let Ok(guard) = self.inner.lock() {
            if let Some(ref state) = *guard {
                return state.lighting_enabled;
            }
        }
        true
    }

    pub fn set_show_fog(&self, show: bool) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.show_fog = show;
            }
        }
    }

    pub fn get_show_fog(&self) -> bool {
        if let Ok(guard) = self.inner.lock() {
            if let Some(ref state) = *guard {
                return state.show_fog;
            }
        }
        false
    }

    pub fn set_show_walkmesh(&self, show: bool) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.show_walkmesh = show;
            }
        }
    }

    pub fn set_show_paint(&self, show: bool) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.show_paint = show;
            }
        }
    }

    pub fn set_clear_color(&self, color: wgpu::Color) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.clear_color = color;
            }
        }
    }

    pub fn set_water(&self, level: i32, show: bool, color: [u8; 4]) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.water_level = level;
                state.show_water = show;
                state.water_color = color;
            }
        }
    }

    pub fn camera_state(&self) -> Option<OrbitCamera> {
        if let Ok(guard) = self.inner.lock() {
            if let Some(ref state) = *guard {
                return Some(state.camera.clone());
            }
        }
        None
    }

    pub fn set_camera(&self, camera: &OrbitCamera) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.camera = camera.clone();
            }
        }
    }

    pub fn set_show_tiles(&self, show: bool) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.show_tiles = show;
            }
        }
    }

    pub fn get_show_tiles(&self) -> bool {
        if let Ok(guard) = self.inner.lock() {
            if let Some(ref state) = *guard {
                return state.show_tiles;
            }
        }
        true
    }

    pub fn walk_camera(&self, forward: f32, right: f32) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.camera.walk(forward, right);
            }
        }
    }

    pub fn rotate_camera(&self, delta_x: f32, delta_y: f32) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.camera.rotate(delta_x, delta_y);
            }
        }
    }

    pub fn zoom_camera(&self, delta: f32) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.camera.zoom(delta);
            }
        }
    }

    pub fn pan_camera(&self, delta_x: f32, delta_y: f32) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut state) = *guard {
                state.camera.pan(delta_x, delta_y);
            }
        }
    }

    pub fn raypick(&self, screen_x: f32, screen_y: f32, view_w: f32, view_h: f32) -> Option<(usize, tile_renderer::MarkerInfo)> {
        if let Ok(guard) = self.inner.lock() {
            if let Some(ref state) = *guard {
                return state.raypick(screen_x, screen_y, view_w, view_h).map(|(i, m)| (i, *m));
            }
        }
        None
    }

    pub fn pick_ground(&self, screen_x: f32, screen_y: f32, view_w: f32, view_h: f32) -> Option<[f32; 3]> {
        if let Ok(guard) = self.inner.lock() {
            if let Some(ref state) = *guard {
                return state.pick_ground(screen_x, screen_y, view_w, view_h);
            }
        }
        None
    }
}

unsafe impl Send for WgpuViewport {}
unsafe impl Sync for WgpuViewport {}

impl CallbackTrait for WgpuViewport {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        // Handle pending area load (called from update thread)
        if let Ok(mut pending_guard) = self.pending.lock() {
            if let Some(pending) = pending_guard.take() {
                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: pending.format,
                    width: screen_descriptor.size_in_pixels[0],
                    height: screen_descriptor.size_in_pixels[1],
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                };
                let existing_camera = self.inner.lock().ok().and_then(|g| g.as_ref().map(|s| s.camera.clone()));
                let vf = self.visibility_filter.lock().map(|g| g.clone()).unwrap_or_default();
                let erf = self.module_erf_bytes.lock().ok()
                    .and_then(|g| g.clone())
                    .and_then(|bytes| nwn_erf::ErfFile::from_bytes(&bytes).ok());
                let mut state = ViewportState::new(device, &config, &pending.area, pending.version, vf, existing_camera,
                    erf.as_ref(),
                    self.mod_path.lock().ok().and_then(|g| g.clone()).as_deref(),
                    &self.hak_names.lock().map(|g| g.clone()).unwrap_or_default(),
                );
                let area_ref = &*pending.area;
                let paint_data = tile_renderer::build_paint_overlay_ex(area_ref, 0, false, [40, 80, 200, 160]);
                let paint_vertex_buf = if !paint_data.vertices.is_empty() {
                    Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("nwn3d_paint_vbuf"),
                        contents: bytemuck::cast_slice(&paint_data.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }))
                } else { None };
                let paint_index_buf = if !paint_data.indices.is_empty() {
                    Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("nwn3d_paint_ibuf"),
                        contents: bytemuck::cast_slice(&paint_data.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    }))
                } else { None };
                state.paint_idx_count = paint_data.indices.len() as u32;
                state.paint_vertex_buf = paint_vertex_buf;
                state.paint_index_buf = paint_index_buf;
                if let Ok(mut guard) = self.inner.lock() {
                    *guard = Some(state);
                }
            }
        }

        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let state = match &mut *guard {
            Some(s) => s,
            None => return Vec::new(),
        };

        state.ensure_textures(device, queue);
        let size = (screen_descriptor.size_in_pixels[0], screen_descriptor.size_in_pixels[1]);
        state.update_uniforms(queue, size);
        state.ensure_scene_textures(device, size.0.max(1), size.1.max(1));

        // Alternate buffer each frame to avoid write/read race on the offscreen texture
        state.frame_index = (state.frame_index + 1) & 1;
        let idx = state.frame_index;
        let color_view = match state.color_view[idx].as_ref() {
            Some(v) => v.clone(),
            None => return Vec::new(),
        };
        let depth_view = match state.depth_view[idx].as_ref() {
            Some(v) => v.clone(),
            None => return Vec::new(),
        };

        // Render 3D scene to offscreen texture with depth buffer
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("nwn3d_scene_encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("nwn3d_scene_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(state.clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            state.render_3d(&mut rpass);
        }

        vec![encoder.finish()]
    }

    fn paint(
        &self,
        info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &CallbackResources,
    ) {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let state = match &*guard {
            Some(s) => s,
            None => return,
        };

        let clip = info.clip_rect_in_pixels();
        render_pass.set_scissor_rect(
            clip.left_px.max(0) as u32,
            clip.top_px.max(0) as u32,
            clip.width_px.max(0) as u32,
            clip.height_px.max(0) as u32,
        );

        if let Some(ref bg) = state.blit_bind_group[state.frame_index] {
            render_pass.set_pipeline(&state.blit_pipeline);
            render_pass.set_bind_group(0, bg, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }

    fn finish_prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _egui_encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        Vec::new()
    }
}

fn create_blit_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    blit_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("nwn3d_blit_shader"),
        source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("nwn3d_blit_layout"),
        bind_group_layouts: &[blit_bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("nwn3d_blit_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent::OVER,
                    alpha: wgpu::BlendComponent::OVER,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview: None,
        cache: None,
    })
}

const BLIT_SHADER: &str = r#"
@group(0) @binding(0) var blit_sampler: sampler;
@group(0) @binding(1) var blit_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var output: VertexOutput;
    let pos = vec2<f32>(
        f32(vi & 1u) * 4.0 - 1.0,
        f32(vi & 2u) * 2.0 - 1.0
    );
    output.position = vec4<f32>(pos, 0.0, 1.0);
    output.uv = vec2<f32>(pos.x * 0.5 + 0.5, -pos.y * 0.5 + 0.5);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(blit_texture, blit_sampler, input.uv);
}
"#;
