use imgui::ColorEdit;
use lifec::editor::{Call, RuntimeEditor, Builder};
use lifec::plugins::{Connection, Remote, Sequence, ThunkContext, Process};
use lifec::{Component, DenseVecStorage, Entity, Extension, Join, Value, WorldExt};
use std::collections::BTreeMap;
use std::ops::DerefMut;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{event, Level};
use wgpu::{DepthStencilState, SurfaceConfiguration};
use wgpu_glyph::{
    ab_glyph, BuiltInLineBreaker, GlyphBrush, GlyphBrushBuilder, HorizontalAlign, Layout, Section,
    Text, VerticalAlign,
};

mod char_device;
pub use char_device::CharDevice;

mod theme;
pub use theme::Theme;
pub use theme::Token;

mod color;
pub use color::ColorTheme;

mod runmd;
pub use runmd::Runmd;

/// Shell extension for the lifec runtime
pub struct Shell<Style = DefaultTheme>
where
    Style: ColorTheme + Default,
{
    /// runtime editor
    runtime_editor: RuntimeEditor,
    /// glyph_brush, for rendering fonts
    brush: Option<GlyphBrush<DepthStencilState>>,
    /// byte receiver
    byte_rx: Option<Receiver<(u32, u8)>>,
    /// byte sender
    byte_tx: Option<Sender<(u32, u8)>>,
    /// char_devices, the first device writes to the shell buffer, and the other devices are for displays
    char_devices: BTreeMap<u32, CharDevice>,
    /// sets the current char_device that can be edited
    editing: Option<u32>,
    /// theme
    theme: Option<Theme<Style>>,
    /// current_output
    channel: i32,
    /// background clear color
    background: [f32; 4],
}

impl<Style> Default for Shell<Style>
where
    Style: ColorTheme + Default,
{
    fn default() -> Self {
        Self {
            runtime_editor: Default::default(),
            brush: Default::default(),
            byte_rx: Default::default(),
            byte_tx: Default::default(),
            char_devices: Default::default(),
            editing: Default::default(),
            theme: Default::default(),
            channel: Default::default(),
            background: Style::background(),
        }
    }
}

#[derive(Default, Clone)]
pub struct DefaultTheme;

impl ColorTheme for DefaultTheme {
    fn prompt() -> Text<'static> {
        Text::new("> ")
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_scale(40.0)
    }

    fn cursor() -> Text<'static> {
        Text::new("_")
            .with_color([0.4, 0.8, 0.8, 1.0])
            .with_scale(40.0)
            .with_z(0.2)
    }

    fn red() -> [f32; 4] {
        [0.7454, 0.14996, 0.17789, 1.0]
    }

    fn blue() -> [f32; 4] {
        [0.11954, 0.42869, 0.86316, 1.0]
    }

    fn purple() -> [f32; 4] {
        [0.56471, 0.18782, 0.72306, 1.0]
    }

    fn green() -> [f32; 4] {
        [0.31399, 0.54572, 0.1912, 1.0]
    }

    fn yellow() -> [f32; 4] {
        [0.78354, 0.52712, 0.19807, 1.0]
    }

    fn background() -> [f32; 4] {
        [0.02122, 0.02519, 0.03434, 1.0]
    }

    fn orange() -> [f32; 4] {
        [0.78354, 0.52712, 0.19807, 1.0]
    }
}

/// This component adds a channel to this shell
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct ShellChannel(Option<Sender<(u32, u8)>>);

impl<Style> Shell<Style>
where
    Style: ColorTheme + Default,
{
    /// gets the underlying runtime_editor
    pub fn runtime_editor_mut(&mut self) -> &mut RuntimeEditor {
        &mut self.runtime_editor
    }

    /// Returns the text brush and char device being edited
    pub fn prepare_render_input(
        &mut self,
    ) -> (
        Option<&mut GlyphBrush<DepthStencilState>>,
        Option<&mut CharDevice>,
        Option<&mut Theme<Style>>,
    ) {
        if let Some(editing) = self.editing {
            if let Some(device) = self.char_devices.get_mut(&editing) {
                (self.brush.as_mut(), Some(device), self.theme.as_mut())
            } else {
                (self.brush.as_mut(), None, self.theme.as_mut())
            }
        } else {
            (None, None, self.theme.as_mut())
        }
    }

    /// Returns the text brush and char device being edited
    pub fn prepare_render_output(
        &mut self,
        channel: u32,
    ) -> (
        Option<&mut GlyphBrush<DepthStencilState>>,
        Option<&mut CharDevice>,
        Option<&mut Theme<Style>>,
    ) {
        if let Some(device) = self.char_devices.get_mut(&channel) {
            (self.brush.as_mut(), Some(device), self.theme.as_mut())
        } else {
            (self.brush.as_mut(), None, self.theme.as_mut())
        }
    }

    /// Returns true if the shell was taken.
    pub fn add_device(&'_ mut self, entity: Entity) -> Option<ShellChannel> {
        if let Some(tx) = self.byte_tx.clone() {
            let channel = entity.id();
            self.char_devices.insert(channel, CharDevice::default());

            Some(ShellChannel(Some(tx)))
        } else {
            None
        }
    }

    /// Renders the input section
    pub fn render_input(&'_ mut self, config: &SurfaceConfiguration) {
        if let (Some(glyph_brush), Some(active), Some(theme)) = self.prepare_render_input() {
            // Renders the buffer
            glyph_brush.queue(Section {
                screen_position: (90.0, 180.0),
                bounds: (config.width as f32 / 2.0, config.height as f32),
                text: theme.render::<Runmd>(active.output().as_ref()),
                layout: Layout::Wrap {
                    line_breaker: BuiltInLineBreaker::AnyCharLineBreaker,
                    h_align: HorizontalAlign::Left,
                    v_align: VerticalAlign::Top,
                },
            });

            // Renders the cursor
            glyph_brush.queue(Section {
                screen_position: (90.0, 180.0),
                bounds: (config.width as f32 / 2.0, config.height as f32),
                text: {
                    vec![
                        // prompt,
                        Text::new(active.before_cursor().as_ref())
                            .with_color([0.0, 0.0, 0.0, 0.0])
                            .with_scale(40.0)
                            .with_z(0.2),
                        Text::new("_")
                            .with_color([0.4, 0.8, 0.8, 1.0])
                            .with_scale(40.0)
                            .with_z(0.2),
                        Text::new(active.after_cursor().as_ref())
                            .with_color([0.0, 0.0, 0.0, 0.0])
                            .with_scale(40.0)
                            .with_z(0.2),
                    ]
                },
                layout: Layout::Wrap {
                    line_breaker: BuiltInLineBreaker::AnyCharLineBreaker,
                    h_align: HorizontalAlign::Left,
                    v_align: VerticalAlign::Top,
                },
            });

            // Renders line numbers
            glyph_brush.queue(Section {
                screen_position: (10.0, 180.0),
                bounds: (config.width as f32 / 2.0, config.height as f32),
                text: {
                    vec![Text::new(active.line_nos().as_ref())
                        .with_color([1.0, 1.0, 1.0, 0.4])
                        .with_scale(40.0)
                        .with_z(1.0)]
                },
                ..Default::default()
            });
        }
    }

    /// Renders the currently active channel
    pub fn render_channel(&mut self, config: &SurfaceConfiguration) {
        if let (Some(glyph_brush), Some(active), Some(theme)) =
            self.prepare_render_output(self.channel as u32)
        {
            glyph_brush.queue(Section {
                screen_position: ((config.width as f32) / 2.0 + 60.0, 180.0),
                bounds: (config.width as f32 / 2.0, config.height as f32),
                text: theme.render::<Runmd>(active.output().as_ref()),
                layout: Layout::Wrap {
                    line_breaker: BuiltInLineBreaker::AnyCharLineBreaker,
                    h_align: HorizontalAlign::Left,
                    v_align: VerticalAlign::Top,
                },
            });
        }
    }
}

impl Extension for Shell {
    fn configure_app_world(_world: &mut lifec::World) {
        _world.register::<ShellChannel>();

        _world.insert(wgpu::Color {
            r: 0.02122,
            g: 0.02519,
            b: 0.03434,
            a: 1.0,
        });

        _world
            .create_entity()
            .with(ThunkContext::default())
            .build();
    }

    fn on_window_event(
        &'_ mut self,
        _app_world: &lifec::World,
        event: &'_ lifec::editor::WindowEvent<'_>,
    ) {
        match (event, self.prepare_render_input()) {
            (lifec::editor::WindowEvent::ReceivedCharacter(char), _) => {
                if let Some(sender) = &self.byte_tx {
                    sender.try_send((0, *char as u8)).ok();
                }
            }
            (
                lifec::editor::WindowEvent::KeyboardInput { input, .. },
                (.., Some(editing), _theme),
            ) => {
                match (input.virtual_keycode, input.state) {
                    // TODO: After integrating some parts from gamegamegame, this part can be improved
                    (Some(key), _) => match key {
                        winit::event::VirtualKeyCode::Left => {
                            editing.cursor_left();
                        }
                        winit::event::VirtualKeyCode::Right => {
                            editing.cursor_right();
                        }
                        winit::event::VirtualKeyCode::Down => {
                            editing.cursor_down();
                        }
                        winit::event::VirtualKeyCode::Up => {
                            editing.cursor_up();
                        }
                        winit::event::VirtualKeyCode::Tab => {
                            if let Some(sender) = &self.byte_tx {
                                sender.try_send((0, ' ' as u8)).ok();
                                sender.try_send((0, ' ' as u8)).ok();
                                sender.try_send((0, ' ' as u8)).ok();
                                sender.try_send((0, ' ' as u8)).ok();
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn on_render_init(
        &mut self,
        _surface: &wgpu::Surface,
        _config: &wgpu::SurfaceConfiguration,
        _adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
        if let Some(inconsolata) =
            ab_glyph::FontArc::try_from_slice(include_bytes!("Inconsolata-Regular.ttf")).ok()
        {
            let glyph_brush = GlyphBrushBuilder::using_font(inconsolata)
                .depth_stencil_state(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                })
                .build(&device, wgpu::TextureFormat::Bgra8UnormSrgb);

            self.brush = Some(glyph_brush);

            let (tx, rx) = channel::<(u32, u8)>(300);
            self.byte_rx = Some(rx);
            self.byte_tx = Some(tx);
            if self.char_devices.is_empty() {
                self.char_devices.insert(0, CharDevice::default());
                self.editing = Some(0);
            }
            let mut default_context = ThunkContext::default();
            default_context
                .as_mut()
                .define("bracket", "color")
                .edit_as(Value::TextBuffer("purple".to_string()));
            default_context
                .as_mut()
                .define("operator", "color")
                .edit_as(Value::TextBuffer("yellow".to_string()));
            default_context
                .as_mut()
                .define("identifier", "color")
                .edit_as(Value::TextBuffer("red".to_string()));
            default_context
                .as_mut()
                .define("keyword", "color")
                .edit_as(Value::TextBuffer("blue".to_string()));
            default_context
                .as_mut()
                .define("literal", "color")
                .edit_as(Value::TextBuffer("green".to_string()));
            default_context
                .as_mut()
                .define("comment", "color")
                .edit_as(Value::TextBuffer("green".to_string()));
            default_context
                .as_mut()
                .define("whitespace", "color")
                .edit_as(Value::TextBuffer("yellow".to_string()));

            self.theme = Some(Theme::new_with(default_context));
        }
    }

    fn on_render(
        &'_ mut self,
        view: &wgpu::TextureView,
        depth_view: Option<&wgpu::TextureView>,
        _surface: &wgpu::Surface,
        config: &wgpu::SurfaceConfiguration,
        _adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        staging_belt: &mut wgpu::util::StagingBelt,
    ) {
        self.render_input(config);

        self.render_channel(config);

        // Draw the text!
        if let Some(depth_view) = depth_view.as_ref() {
            if let Some(brush) = self.brush.as_mut() {
                brush
                    .draw_queued(
                        device,
                        staging_belt,
                        encoder,
                        view,
                        wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(-1.0),
                                store: true,
                            }),
                            stencil_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(0),
                                store: true,
                            }),
                        },
                        config.width,
                        config.height,
                    )
                    .expect("Draw queued");
            }
        }
    }

    fn on_run(&'_ mut self, app_world: &lifec::World) {
        if let Some(rx) = self.byte_rx.as_mut() {
            if let Some((channel, next)) = rx.try_recv().ok() {
                event!(Level::TRACE, "received {next} for char_device {channel}");
                if let Some(char_device) = self.char_devices.get_mut(&channel) {
                    char_device.write(next);
                }
            }
        }

        let mut shell_outputs = app_world.write_component::<ShellChannel>();
        let mut contexts = app_world.write_component::<ThunkContext>();
        let entities = app_world.entities();

        for (entity, shell_output, context) in (&entities, &mut shell_outputs, &mut contexts).join()
        {
            if let ShellChannel(None) = shell_output {
                if let Some(channel) = self.add_device(entity) {
                    *shell_output = channel;
                }
            } else if let ShellChannel(Some(channel)) = shell_output {
                context.set_char_device(channel.clone());
            }
        }
    }

    fn on_ui(&'_ mut self, app_world: &lifec::World, ui: &'_ imgui::Ui<'_>) {
        ui.main_menu_bar(|| {
            ui.menu("Shell", || {
                if let Some(theme) = self.theme.as_mut() {
                    for (token, color) in theme.colors_mut() {
                        ColorEdit::new(format!("{:?}", token), color).build(ui);
                    }
                }

                if ColorEdit::new("Background clear", &mut self.background).build(ui) {
                    let [r, g, b, a] = self.background;
                    let mut clear_color = app_world.write_resource::<wgpu::Color>();
                    let clear_color = clear_color.deref_mut();
                    *clear_color = wgpu::Color {
                        r: r.into(),
                        g: g.into(),
                        b: b.into(),
                        a: a.into(),
                    };
                }

                if ui.button("Reset colors") {
                    if let Some(theme) = self.theme.as_mut() {
                        theme.reset_colors();

                        if let Some(color) =
                            theme.get_color(Token::Custom("background".to_string()))
                        {
                            self.background = *color;
                            let [r, g, b, a] = self.background;
                            let mut clear_color = app_world.write_resource::<wgpu::Color>();
                            let clear_color = clear_color.deref_mut();
                            *clear_color = wgpu::Color {
                                r: r.into(),
                                g: g.into(),
                                b: b.into(),
                                a: a.into(),
                            };
                        }
                    }
                }

                ui.separator();
                if ui
                    .input_int("Current output channel", &mut self.channel)
                    .build()
                {}

                if ui.button("Add Remote") {
                    if let Some(created) = self
                        .runtime_editor
                        .runtime_mut()
                        .schedule_with_engine::<Call, Process>(app_world, "shell")
                    {
                        if let Some(channel) = self.add_device(created) {
                            app_world.write_component().insert(created, channel).ok();
                            app_world
                                .write_component()
                                .insert(created, Sequence::default())
                                .ok();
                            app_world
                                .write_component()
                                .insert(created, Connection::default())
                                .ok();
                        }
                    }
                }
            });
        });
    }
}
