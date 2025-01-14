use std::hash::Hash;

use web_time::{Duration, Instant};

use eframe::egui;
use noise_functions_config::{
    noise_functions::from_fast_noise_2::cell::{CellIndex, DistanceFn, DistanceReturnType},
    Config, Fractal, Improve, Noise,
};

pub struct App {
    settings: Settings,
    texture: egui::TextureHandle,
    changed: bool,
    elapsed: Duration,
    sample_success: bool,

    // we cache the vecs so we don't need to allocate them each update
    cache: Cache,
}

struct Settings {
    config: Config,
    texture_size: usize,
    dimension: Dimension,
    x: f32,
    y: f32,
    z: f32,
    w: f32,
    simd: bool,
    show_tiles: bool,
    link_tile_size_to_frequency: bool,
}

#[derive(Default)]
struct Cache {
    values: Vec<f32>,
    pixels: Vec<egui::Color32>,
    size: usize,
}

impl Cache {
    fn resize(&mut self, new_size: usize) {
        if new_size == self.size {
            return;
        }

        self.values = Vec::new();
        self.pixels = Vec::new();
        self.values.resize(new_size, 0.0);
        self.pixels
            .resize(new_size, egui::Color32::from_rgb(255, 0, 255));
        self.size = new_size;
    }
}

const DEFAULT_CONFIG: Config = Config {
    noise: Noise::Simplex,
    seed: 0,
    frequency: 3.0,

    // fractal
    fractal: Fractal::None,
    lacunarity: 2.0,
    octaves: 3,
    gain: 0.5,
    ping_pong_strength: 2.0,
    weighted_strength: 0.0,

    // open simplex 2
    improve: Improve::Xy,

    // cell
    jitter: 1.0,
    value_index: CellIndex::I0,
    distance_fn: DistanceFn::Euclidean,
    distance_indices: [CellIndex::I0, CellIndex::I1],
    distance_return_type: DistanceReturnType::Index0,

    // tiling
    tileable: true,
    tile_width: 3.0,
    tile_height: 3.0,
};

const DEFAULT_SETTINGS: Settings = Settings {
    config: DEFAULT_CONFIG,
    texture_size: 295,
    dimension: Dimension::D2,
    x: 0.0,
    y: 0.0,
    z: 0.0,
    w: 0.0,
    simd: false,
    show_tiles: true,
    link_tile_size_to_frequency: true,
};

#[cfg(debug_assertions)]
const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"), " (debug)");

#[cfg(not(debug_assertions))]
const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    D2,
    D3,
    D4,
}

impl Dimension {
    pub const VARIANTS: &'static [Self] = &[Self::D2, Self::D3, Self::D4];

    pub fn to_str(self) -> &'static str {
        match self {
            Dimension::D2 => "2D",
            Dimension::D3 => "3D",
            Dimension::D4 => "4D",
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            settings: DEFAULT_SETTINGS,
            texture: cc.egui_ctx.load_texture(
                "noise",
                egui::ColorImage::example(),
                egui::TextureOptions::NEAREST,
            ),
            changed: true,
            elapsed: Duration::from_nanos(0),
            cache: Default::default(),
            sample_success: true,
        }
    }

    pub fn settings_panel_contents(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let Self {
            settings:
                Settings {
                    config,
                    x,
                    y,
                    z,
                    w,
                    simd,
                    show_tiles,
                    link_tile_size_to_frequency,
                    dimension,
                    texture_size,
                },
            changed,
            ..
        } = self;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.heading("Noise Functions Demo");
            ui.hyperlink_to(
                egui::RichText::new(format!(" on GitHub {}", egui::special_emojis::GITHUB))
                    .heading(),
                "https://github.com/bluurryy/noise-functions-demo",
            );
        });

        ui.separator();

        egui::Grid::new(0)
            .striped(true)
            .min_col_width(0.0)
            .num_columns(3)
            .show(ui, |ui| {
                macro_rules! combo_box {
                    ($id:literal, $ty:ident) => {
                        |value| SimpleComboBox {
                            id: $id,
                            value,
                            variants: $ty::VARIANTS,
                            to_str: $ty::to_str,
                        }
                    };
                }

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Type",
                        value: &mut config.noise,
                        default: DEFAULT_CONFIG.noise,
                        widget: combo_box!("noise type", Noise),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Dimension",
                        value: dimension,
                        default: DEFAULT_SETTINGS.dimension,
                        widget: combo_box!("dimension", Dimension),
                    },
                );

                if matches!(config.noise, Noise::OpenSimplex2 | Noise::OpenSimplex2s)
                    && matches!(dimension, Dimension::D3)
                {
                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Improve",
                            value: &mut config.improve,
                            default: DEFAULT_CONFIG.improve,
                            widget: combo_box!("improve", Improve),
                        },
                    );
                }

                if matches!(
                    config.noise,
                    Noise::CellValue
                        | Noise::CellDistance
                        | Noise::FastCellValue
                        | Noise::FastCellDistance
                        | Noise::FastCellDistanceSq
                ) {
                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Jitter",
                            value: &mut config.jitter,
                            default: DEFAULT_CONFIG.jitter,
                            widget: |v| egui::DragValue::new(v).speed(0.02),
                        },
                    );
                }

                if matches!(config.noise, Noise::CellValue | Noise::CellDistance) {
                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Distance Function",
                            value: &mut config.distance_fn,
                            default: DEFAULT_CONFIG.distance_fn,
                            widget: combo_box!("distance fn", DistanceFn),
                        },
                    );

                    if matches!(config.noise, Noise::CellValue | Noise::FastCellValue) {
                        setting(
                            changed,
                            ui,
                            Setting {
                                name: "Value Index",
                                value: &mut config.value_index,
                                default: DEFAULT_CONFIG.value_index,
                                widget: combo_box!("value index", CellIndex),
                            },
                        );
                    }

                    if matches!(
                        config.noise,
                        Noise::CellDistance | Noise::FastCellDistance | Noise::FastCellDistanceSq
                    ) {
                        setting(
                            changed,
                            ui,
                            Setting {
                                name: "Distance Index 0",
                                value: &mut config.distance_indices[0],
                                default: DEFAULT_CONFIG.distance_indices[0],
                                widget: combo_box!("distance index 0", CellIndex),
                            },
                        );

                        setting(
                            changed,
                            ui,
                            Setting {
                                name: "Distance Index 1",
                                value: &mut config.distance_indices[1],
                                default: DEFAULT_CONFIG.distance_indices[1],
                                widget: combo_box!("distance index 1", CellIndex),
                            },
                        );

                        setting(
                            changed,
                            ui,
                            Setting {
                                name: "Distance Return Type",
                                value: &mut config.distance_return_type,
                                default: DEFAULT_CONFIG.distance_return_type,
                                widget: combo_box!("distance return type", DistanceReturnType),
                            },
                        );
                    }
                }

                setting_separator(ui);

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Fractal",
                        value: &mut config.fractal,
                        default: DEFAULT_CONFIG.fractal,
                        widget: combo_box!("fractal", Fractal),
                    },
                );

                if config.fractal != Fractal::None {
                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Octaves",
                            value: &mut config.octaves,
                            default: DEFAULT_CONFIG.octaves,
                            widget: |v| egui::DragValue::new(v).speed(0.02).range(1..=8),
                        },
                    );

                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Lacunarity",
                            value: &mut config.lacunarity,
                            default: DEFAULT_CONFIG.lacunarity,
                            widget: |v| egui::DragValue::new(v).speed(0.02),
                        },
                    );

                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Gain",
                            value: &mut config.gain,
                            default: DEFAULT_CONFIG.gain,
                            widget: |v| egui::DragValue::new(v).speed(0.02),
                        },
                    );

                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Weighted Strength",
                            value: &mut config.weighted_strength,
                            default: DEFAULT_CONFIG.weighted_strength,
                            widget: |v| egui::Slider::new(v, 0.0..=1.0),
                        },
                    );

                    if config.fractal == Fractal::PingPong {
                        setting(
                            changed,
                            ui,
                            Setting {
                                name: "Ping Pong Strength",
                                value: &mut config.ping_pong_strength,
                                default: DEFAULT_CONFIG.ping_pong_strength,
                                widget: |v| egui::Slider::new(v, 0.5..=3.0),
                            },
                        );
                    }
                }

                setting_separator(ui);

                if setting(
                    changed,
                    ui,
                    Setting {
                        name: "Frequency",
                        value: &mut config.frequency,
                        default: DEFAULT_CONFIG.frequency,
                        widget: |v| egui::DragValue::new(v).speed(0.02),
                    },
                ) && *link_tile_size_to_frequency
                {
                    config.tile_width = config.frequency;
                    config.tile_height = config.frequency;
                }

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Seed",
                        value: &mut config.seed,
                        default: DEFAULT_CONFIG.seed,
                        widget: |v| egui::DragValue::new(v).speed(0.1),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Tileable",
                        value: &mut config.tileable,
                        default: DEFAULT_CONFIG.tileable,
                        widget: egui::Checkbox::without_text,
                    },
                );

                if config.tileable {
                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Link Tile Size to Freq.",
                            value: link_tile_size_to_frequency,
                            default: DEFAULT_SETTINGS.link_tile_size_to_frequency,
                            widget: egui::Checkbox::without_text,
                        },
                    );

                    if setting(
                        changed,
                        ui,
                        Setting {
                            name: "Tile Width",
                            value: &mut config.tile_width,
                            default: DEFAULT_CONFIG.tile_width,
                            widget: |v| egui::DragValue::new(v).speed(0.02),
                        },
                    ) && *link_tile_size_to_frequency
                    {
                        config.tile_height = config.tile_width;
                        config.frequency = config.tile_width;
                    }

                    if setting(
                        changed,
                        ui,
                        Setting {
                            name: "Tile Height",
                            value: &mut config.tile_height,
                            default: DEFAULT_CONFIG.tile_height,
                            widget: |v| egui::DragValue::new(v).speed(0.02),
                        },
                    ) && *link_tile_size_to_frequency
                    {
                        config.tile_width = config.tile_height;
                        config.frequency = config.tile_height;
                    }
                }

                setting_separator(ui);

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Texture Size",
                        value: texture_size,
                        default: DEFAULT_SETTINGS.texture_size,
                        widget: |v| egui::DragValue::new(v).range(0..=1024),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "X",
                        value: x,
                        default: DEFAULT_SETTINGS.x,
                        widget: |v| egui::DragValue::new(v).speed(0.002),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Y",
                        value: y,
                        default: DEFAULT_SETTINGS.y,
                        widget: |v| egui::DragValue::new(v).speed(0.002),
                    },
                );

                if matches!(dimension, Dimension::D3 | Dimension::D4) {
                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Z",
                            value: z,
                            default: DEFAULT_SETTINGS.z,
                            widget: |v| egui::DragValue::new(v).speed(0.002),
                        },
                    );
                }

                if matches!(dimension, Dimension::D4) {
                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "W",
                            value: w,
                            default: DEFAULT_SETTINGS.w,
                            widget: |v| egui::DragValue::new(v).speed(0.002),
                        },
                    );
                }

                if matches!(dimension, Dimension::D2) && config.tileable {
                    setting(
                        changed,
                        ui,
                        Setting {
                            name: "Show Tiles",
                            value: show_tiles,
                            default: DEFAULT_SETTINGS.show_tiles,
                            widget: egui::Checkbox::without_text,
                        },
                    );
                }

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Simd",
                        value: simd,
                        default: DEFAULT_SETTINGS.simd,
                        widget: egui::Checkbox::without_text,
                    },
                );
            });

        ui.add_space(5.0);
    }

    pub fn image_preview_contents(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let Self {
            settings,
            texture,
            changed,
            cache,
            ..
        } = self;

        if *changed {
            *changed = false;

            let size = settings.texture_size;
            let z = settings.z;
            let w = settings.w;

            cache.resize(size * size);

            let start = Instant::now();

            fn sample(values: &mut [f32], settings: &Settings, f: impl Fn(f32, f32) -> f32) {
                let Settings {
                    config: Config { tileable, .. },
                    texture_size: size,
                    x: x_shift,
                    y: y_shift,
                    ..
                } = *settings;

                let scalar = 1.0 / size as f32;

                if tileable {
                    for y in 0..size {
                        for x in 0..size {
                            let i = y * size + x;
                            let x = x as f32 * scalar + x_shift;
                            let y = y as f32 * scalar + y_shift;
                            values[i] = f(x, y);
                        }
                    }
                } else {
                    let scalar_times_two = scalar * 2.0;

                    for y in 0..size {
                        for x in 0..size {
                            let i = y * size + x;
                            let x = x as f32 * scalar_times_two - 1.0 + x_shift;
                            let y = y as f32 * scalar_times_two - 1.0 + y_shift;
                            values[i] = f(x, y);
                        }
                    }
                }
            }

            let sampled: bool = if settings.simd {
                match settings.dimension {
                    Dimension::D2 => {
                        if let Some(sampler) = settings.config.sampler2a() {
                            sample(&mut cache.values, settings, |x, y| {
                                sampler.sample([x, y].into())
                            });
                            true
                        } else {
                            false
                        }
                    }
                    Dimension::D3 => {
                        if let Some(sampler) = settings.config.sampler3a() {
                            sample(&mut cache.values, settings, |x, y| {
                                sampler.sample([x, y, z, 0.0].into())
                            });
                            true
                        } else {
                            false
                        }
                    }
                    Dimension::D4 => {
                        if let Some(sampler) = settings.config.sampler4a() {
                            sample(&mut cache.values, settings, |x, y| {
                                sampler.sample([x, y, z, w].into())
                            });
                            true
                        } else {
                            false
                        }
                    }
                }
            } else {
                match settings.dimension {
                    Dimension::D2 => {
                        if let Some(sampler) = settings.config.sampler2() {
                            sample(&mut cache.values, settings, |x, y| sampler.sample([x, y]));
                            true
                        } else {
                            false
                        }
                    }
                    Dimension::D3 => {
                        if let Some(sampler) = settings.config.sampler3() {
                            sample(&mut cache.values, settings, |x, y| {
                                sampler.sample([x, y, z])
                            });
                            true
                        } else {
                            false
                        }
                    }
                    Dimension::D4 => {
                        if let Some(sampler) = settings.config.sampler4() {
                            sample(&mut cache.values, settings, |x, y| {
                                sampler.sample([x, y, z, w])
                            });
                            true
                        } else {
                            false
                        }
                    }
                }
            };

            self.sample_success = sampled;
            self.elapsed = start.elapsed();

            for x in 0..size {
                for y in 0..size {
                    let i = x * size + y;
                    let value = cache.values[i];

                    let value_01 = match settings.config.noise {
                        Noise::Value
                        | Noise::ValueCubic
                        | Noise::Perlin
                        | Noise::Simplex
                        | Noise::OpenSimplex2
                        | Noise::OpenSimplex2s
                        | Noise::CellValue
                        | Noise::FastCellValue => value * 0.5 + 0.5,
                        Noise::CellDistance
                        | Noise::FastCellDistance
                        | Noise::FastCellDistanceSq => value,
                    };

                    let value_255 = (value_01 * 255.0) as u8;
                    let color = egui::Color32::from_gray(value_255);
                    cache.pixels[i] = color;
                }
            }

            texture.set(
                egui::ColorImage {
                    size: [settings.texture_size; 2],
                    pixels: cache.pixels.clone(),
                },
                egui::TextureOptions::NEAREST,
            );
        }

        let size = texture.size_vec2();

        if self.settings.show_tiles && self.sample_success {
            egui::Grid::new("image grid")
                .spacing([0.0; 2])
                .show(ui, |ui| {
                    for i in 0..4 {
                        let sized_texture = egui::load::SizedTexture::new(&mut *texture, size);
                        let image = ui.add(egui::Image::new(sized_texture).fit_to_exact_size(size));

                        ui.painter()
                            .circle_filled(image.rect.center(), 40.0, egui::Color32::BLACK);

                        let galley = ui.painter().layout_no_wrap(
                            i.to_string(),
                            egui::FontId {
                                size: 64.0,
                                family: egui::FontFamily::Proportional,
                            },
                            egui::Color32::WHITE,
                        );

                        ui.painter().galley(
                            image.rect.center() - galley.rect.size() * 0.5,
                            galley,
                            egui::Color32::DEBUG_COLOR,
                        );

                        if i % 2 != 0 {
                            ui.end_row();
                        }
                    }
                });
        } else {
            let sized_texture = egui::load::SizedTexture::new(&mut *texture, size);
            let image = ui.add(egui::Image::new(sized_texture).fit_to_exact_size(size));

            if !self.sample_success {
                let image_rect = egui::Rect::from_min_size(image.rect.left_top(), size);

                let text = "dimension/tileable not available for this noise type";

                let galley = ui.painter().layout_job(egui::text::LayoutJob {
                    sections: vec![egui::text::LayoutSection {
                        leading_space: 0.0,
                        byte_range: 0..text.len(),
                        format: egui::text::TextFormat::simple(
                            egui::FontId {
                                size: 14.0,
                                family: egui::FontFamily::Proportional,
                            },
                            egui::Color32::WHITE,
                        ),
                    }],
                    text: text.into(),
                    wrap: egui::text::TextWrapping {
                        max_width: 200.0,
                        ..Default::default()
                    },
                    halign: egui::Align::Center,
                    ..Default::default()
                });

                ui.painter().rect_filled(
                    egui::Rect::from_center_size(
                        image_rect.center(),
                        galley.rect.size() + egui::Vec2::splat(10.0),
                    ),
                    5.0,
                    egui::Color32::BLACK,
                );

                ui.painter().galley(
                    egui::Rect::from_center_size(image_rect.center(), galley.rect.size())
                        .center_top(),
                    galley,
                    egui::Color32::DEBUG_COLOR,
                );
            }
        }
    }
}

pub fn is_mobile(ctx: &egui::Context) -> bool {
    let screen_size = ctx.screen_rect().size();
    screen_size.x < 550.0
}

const COMBO_BOX_WIDTH: f32 = 150.0;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let is_mobile = is_mobile(ctx);

        egui::SidePanel::left("settings_panel")
            .resizable(false)
            .max_width(325.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(6.0);
                    self.settings_panel_contents(ui, frame);

                    if is_mobile {
                        self.image_preview_contents(ui, frame);
                    } else {
                        ui.separator();
                    }

                    ui.add(
                        egui::Label::new(format!("elapsed: {:?}", self.elapsed)).selectable(false),
                    );
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !is_mobile {
                const TOP_LEFT_JUSTIFIED: egui::Layout = egui::Layout {
                    main_dir: egui::Direction::TopDown,
                    main_wrap: false,
                    main_align: egui::Align::Min,
                    main_justify: true,
                    cross_align: egui::Align::Min,
                    cross_justify: true,
                };

                ui.with_layout(TOP_LEFT_JUSTIFIED, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        self.image_preview_contents(ui, frame);
                    });
                });
            }
        });

        egui::Window::new("")
            .anchor(egui::Align2::RIGHT_BOTTOM, [-5.0, -5.0])
            .interactable(false)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .auto_sized()
            .title_bar(false)
            .frame(egui::Frame {
                inner_margin: egui::Margin::same(2.0),
                outer_margin: egui::Margin::ZERO,
                rounding: egui::Rounding::ZERO,
                shadow: egui::epaint::Shadow::NONE,
                fill: egui::Color32::TRANSPARENT,
                stroke: egui::epaint::Stroke::NONE,
            })
            .show(ctx, |ui| {
                ui.add(egui::Label::new(VERSION).selectable(false));
            });
    }
}

fn setting(changed: &mut bool, ui: &mut egui::Ui, setting: impl egui::Widget) -> bool {
    let setting_changed = ui.add(setting).changed();
    *changed |= setting_changed;
    setting_changed
}

fn setting_separator(ui: &mut egui::Ui) {
    ui.separator();
    ui.separator();
    ui.separator();
    ui.end_row();
}

pub struct Setting<'v, T, W> {
    name: &'static str,
    value: &'v mut T,
    default: T,
    widget: fn(&'v mut T) -> W,
}

impl<T, W> egui::Widget for Setting<'_, T, W>
where
    W: egui::Widget,
    T: PartialEq,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Setting {
            name,
            value,
            default,
            widget,
        } = self;

        ui.add(egui::Label::new(name).selectable(false));
        let response = ui.add(Reset::new(value, default)) | ui.add(widget(value));
        ui.end_row();
        response
    }
}

pub struct SimpleComboBox<'v, T: 'static> {
    id: &'static str,
    value: &'v mut T,
    variants: &'static [T],
    to_str: fn(T) -> &'static str,
}

impl<T> egui::Widget for SimpleComboBox<'_, T>
where
    T: PartialEq + Copy,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self {
            id,
            value,
            variants,
            to_str,
        } = self;

        let egui::InnerResponse {
            inner,
            mut response,
        } = egui::ComboBox::from_id_salt(id)
            .width(COMBO_BOX_WIDTH)
            .selected_text(to_str(*value))
            .show_ui(ui, |ui| {
                let mut changed = false;

                for &variant in variants {
                    changed |= ui
                        .selectable_value(value, variant, to_str(variant))
                        .changed();
                }

                changed
            });

        if inner == Some(true) {
            response.mark_changed();
        }

        response
    }
}

pub struct Reset<'v, T> {
    value: &'v mut T,
    default: T,
}

impl<'v, T> Reset<'v, T> {
    pub fn new(value: &'v mut T, default: T) -> Self {
        Self { value, default }
    }
}

impl<T: PartialEq> egui::Widget for Reset<'_, T> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Reset { value, default } = self;
        let mut response = ui.add_enabled(*value != default, egui::Button::new("⟲"));

        if response.clicked() {
            *value = default;
            response.changed = true;
        }

        response
    }
}
