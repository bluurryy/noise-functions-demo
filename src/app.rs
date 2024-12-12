use std::hash::Hash;

use web_time::{Duration, Instant};

use eframe::egui;
use noise_functions_config::{Config, Fractal, Improve, Noise};

pub struct App {
    config: Config,
    texture: egui::TextureHandle,
    texture_size: usize,
    changed: bool,
    dimension: Dimension,
    z: f32,
    simd: bool,
    elapsed: Duration,

    // we cache the vecs so we don't need to allocate them each update
    cache: Cache,
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
    noise: Noise::OpenSimplex2,
    fractal: Fractal::Fbm,
    improve: Improve::Xy,
    lacunarity: 2.0,
    octaves: 3,
    gain: 0.5,
    ping_pong_strength: 2.0,
    weighted_strength: 0.0,
    frequency: 3.0,
    seed: 0,
    jitter: 1.0,
};

const DEFAULT_TEXTURE_SIZE: usize = 295;
const DEFAULT_DIMENSION: Dimension = Dimension::D2;
const DEFAULT_Z: f32 = 0.0;
const DEFAULT_SIMD: bool = false;

#[cfg(debug_assertions)]
const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"), " (debug)");

#[cfg(not(debug_assertions))]
const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    D2,
    D3,
}

impl Dimension {
    pub const VARIANTS: &'static [Self] = &[Self::D2, Self::D3];

    pub fn to_str(self) -> &'static str {
        match self {
            Dimension::D2 => "2D",
            Dimension::D3 => "3D",
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            config: DEFAULT_CONFIG,
            texture: cc.egui_ctx.load_texture(
                "noise",
                egui::ColorImage::example(),
                egui::TextureOptions::NEAREST,
            ),
            texture_size: DEFAULT_TEXTURE_SIZE,
            dimension: DEFAULT_DIMENSION,
            z: DEFAULT_Z,
            simd: DEFAULT_SIMD,
            changed: true,
            elapsed: Duration::from_nanos(0),
            cache: Default::default(),
        }
    }

    pub fn settings_panel_contents(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let Self {
            config,
            texture_size,
            changed,
            dimension,
            z,
            simd,
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
                        enabled: true,
                        value: &mut config.noise,
                        default: DEFAULT_CONFIG.noise,
                        widget: combo_box!(0, Noise),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Dimension",
                        enabled: true,
                        value: dimension,
                        default: DEFAULT_DIMENSION,
                        widget: combo_box!(1, Dimension),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Improve",
                        enabled: matches!(config.noise, Noise::OpenSimplex2 | Noise::OpenSimplex2s)
                            && !matches!(*dimension, Dimension::D2),
                        value: &mut config.improve,
                        default: DEFAULT_CONFIG.improve,
                        widget: combo_box!(2, Improve),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Jitter",
                        enabled: matches!(
                            config.noise,
                            Noise::CellValue | Noise::CellDistance | Noise::CellDistanceSq
                        ),
                        value: &mut config.jitter,
                        default: DEFAULT_CONFIG.jitter,
                        widget: |v| egui::DragValue::new(v).speed(0.02),
                    },
                );

                setting_separator(ui);

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Fractal",
                        enabled: true,
                        value: &mut config.fractal,
                        default: DEFAULT_CONFIG.fractal,
                        widget: combo_box!(3, Fractal),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Octaves",
                        enabled: config.fractal != Fractal::None,
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
                        enabled: config.fractal != Fractal::None,
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
                        enabled: config.fractal != Fractal::None,
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
                        enabled: config.fractal != Fractal::None,
                        value: &mut config.weighted_strength,
                        default: DEFAULT_CONFIG.weighted_strength,
                        widget: |v| egui::Slider::new(v, 0.0..=1.0),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Ping Pong Strength",
                        enabled: config.fractal == Fractal::PingPong,
                        value: &mut config.ping_pong_strength,
                        default: DEFAULT_CONFIG.ping_pong_strength,
                        widget: |v| egui::Slider::new(v, 0.5..=3.0),
                    },
                );

                setting_separator(ui);

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Frequency",
                        enabled: true,
                        value: &mut config.frequency,
                        default: DEFAULT_CONFIG.frequency,
                        widget: |v| egui::DragValue::new(v).speed(0.02),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Seed",
                        enabled: true,
                        value: &mut config.seed,
                        default: DEFAULT_CONFIG.seed,
                        widget: |v| egui::DragValue::new(v).speed(0.1),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Texture Size",
                        enabled: true,
                        value: texture_size,
                        default: DEFAULT_TEXTURE_SIZE,
                        widget: |v| egui::DragValue::new(v).range(0..=1024),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Z",
                        enabled: !matches!(*dimension, Dimension::D2),
                        value: z,
                        default: DEFAULT_Z,
                        widget: |v| egui::DragValue::new(v).speed(0.002),
                    },
                );

                setting(
                    changed,
                    ui,
                    Setting {
                        name: "Simd",
                        enabled: true,
                        value: simd,
                        default: DEFAULT_SIMD,
                        widget: egui::Checkbox::without_text,
                    },
                );
            });

        ui.add_space(5.0);
    }

    pub fn image_preview_contents(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let Self {
            config,
            texture,
            texture_size,
            changed,
            dimension,
            z,
            simd,
            cache,
            ..
        } = self;

        if *changed {
            *changed = false;

            let size = *texture_size;
            let z = *z;

            cache.resize(size * size);

            let scalar = 1.0 / size as f32;
            let scalar_times_2 = scalar * 2.0;
            let start = Instant::now();

            if *simd {
                if *dimension == Dimension::D2 {
                    let sampler = config.sampler2a();

                    for x in 0..size {
                        for y in 0..size {
                            let i = x * size + y;
                            let x = x as f32 * scalar_times_2 - 1.0;
                            let y = y as f32 * scalar_times_2 - 1.0;

                            cache.values[i] = sampler.sample([x, y].into());
                        }
                    }
                } else {
                    let sampler = config.sampler3a();

                    for x in 0..size {
                        for y in 0..size {
                            let i = x * size + y;
                            let x = x as f32 * scalar_times_2 - 1.0;
                            let y = y as f32 * scalar_times_2 - 1.0;

                            cache.values[i] = sampler.sample([x, y, z, 0.0].into());
                        }
                    }
                }
            } else {
                if *dimension == Dimension::D2 {
                    let sampler = config.sampler2();

                    for x in 0..size {
                        for y in 0..size {
                            let i = x * size + y;
                            let x = x as f32 * scalar_times_2 - 1.0;
                            let y = y as f32 * scalar_times_2 - 1.0;

                            cache.values[i] = sampler.sample([x, y]);
                        }
                    }
                } else {
                    let sampler = config.sampler3();

                    for x in 0..size {
                        for y in 0..size {
                            let i = x * size + y;
                            let x = x as f32 * scalar_times_2 - 1.0;
                            let y = y as f32 * scalar_times_2 - 1.0;

                            cache.values[i] = sampler.sample([x, y, z]);
                        }
                    }
                }
            }

            self.elapsed = start.elapsed();

            for x in 0..size {
                for y in 0..size {
                    let i = x * size + y;
                    let value = cache.values[i];
                    let value_255 = ((value * 0.5 + 0.5) * 255.0) as u8;
                    let color = egui::Color32::from_gray(value_255);
                    cache.pixels[i] = color;
                }
            }

            texture.set(
                egui::ColorImage {
                    size: [*texture_size; 2],
                    pixels: cache.pixels.clone(),
                },
                egui::TextureOptions::NEAREST,
            );
        }

        let size = texture.size_vec2();
        let sized_texture = egui::load::SizedTexture::new(texture, size);
        ui.add(egui::Image::new(sized_texture).fit_to_exact_size(size));
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

fn setting(changed: &mut bool, ui: &mut egui::Ui, setting: impl egui::Widget) {
    *changed |= ui.add(setting).changed();
}

fn setting_separator(ui: &mut egui::Ui) {
    ui.separator();
    ui.separator();
    ui.separator();
    ui.end_row();
}

pub struct Setting<'v, T, W> {
    name: &'static str,
    enabled: bool,
    value: &'v mut T,
    default: T,
    widget: fn(&'v mut T) -> W,
}

impl<'v, T, W> egui::Widget for Setting<'_, T, W>
where
    W: egui::Widget,
    T: PartialEq,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Setting {
            name,
            enabled,
            value,
            default,
            widget,
        } = self;

        ui.add_enabled(enabled, egui::Label::new(name).selectable(false));
        let response = ui.add(Reset::new(value, default)) | ui.add(widget(value));
        ui.end_row();
        response
    }
}

pub struct SimpleComboBox<'v, I, T: 'static> {
    id: I,
    value: &'v mut T,
    variants: &'static [T],
    to_str: fn(T) -> &'static str,
}

impl<I, T> egui::Widget for SimpleComboBox<'_, I, T>
where
    I: Hash,
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
