use imgui::ColorEdit;
use lifec::plugins::ThunkContext;
use lifec::AttributeIndex;
use lifec::{
    Component, DenseVecStorage, Entities, Entity, Extension, Join, System, WorldExt, WriteStorage,
};
use std::collections::BTreeMap;
use std::ops::DerefMut;
use theme::Grammer;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{event, Level};
use wgpu::{DepthStencilState, SurfaceConfiguration};
use wgpu_glyph::{
    ab_glyph, BuiltInLineBreaker, GlyphBrush, GlyphBrushBuilder, HorizontalAlign, Layout, Section,
    Text, VerticalAlign,
};
use winit::event::WindowEvent;

mod char_device;
pub use char_device::CharDevice;

mod theme;
pub use theme::DefaultTheme;
pub use theme::Theme;
pub use theme::Token;

mod color;
pub use color::ColorTheme;

mod runmd;
pub use runmd::Runmd;

mod plain;
pub use plain::Plain;

/// Shell extension for the lifec runtime
pub struct Shell<G, Style = DefaultTheme>
where
    Style: ColorTheme + Default,
    G: Grammer,
{
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
    /// Current-live connection
    connection: Option<TcpStream>,
    /// Address to connect to
    address: Option<String>,
    /// Grammer state
    grammer: G, 
}

impl<G, Style> Default for Shell<G, Style>
where
    Style: ColorTheme + Default,
    G: Grammer + Default,
{
    fn default() -> Self {
        Self {
            brush: Default::default(),
            byte_rx: Default::default(),
            byte_tx: Default::default(),
            char_devices: Default::default(),
            editing: Default::default(),
            theme: Default::default(),
            channel: Default::default(),
            background: Style::background(),
            connection: None,
            address: None,
            grammer: G::default(),
        }
    }
}

/// This component adds a channel to this shell
#[derive(Component, Default, Clone)]
#[storage(DenseVecStorage)]
pub struct ShellChannel(Option<Sender<(u32, u8)>>);

impl<G, Style> Shell<G, Style>
where
    Style: ColorTheme + Default,
    G: Grammer + Clone,
{
    /// Connects to a tcp stream
    pub async fn connect_to(&mut self, address: impl AsRef<str>) {
        self.connection = TcpStream::connect(address.as_ref()).await.ok()
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

    /// Returns devices to render output from a chardevice
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

            event!(Level::DEBUG, "Adding channel for {}", entity.id());
            Some(ShellChannel(Some(tx)))
        } else {
            None
        }
    }

    /// Renders the input section
    pub fn render_input(&'_ mut self, config: &SurfaceConfiguration) {
        let prompt_enabled = self.connection.is_some();
        let grammer = self.grammer.clone();
        if let (Some(glyph_brush), Some(active), Some(theme)) = self.prepare_render_input() {
            // Renders the buffer
            glyph_brush.queue(Section {
                screen_position: (90.0, 180.0),
                bounds: (config.width as f32 / 2.0, config.height as f32),
                // TODO: need to figure out a way to make this generic, but for now this is good enough
                text: theme.render(&grammer, active.output().as_ref(), prompt_enabled),
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
                text: theme.render_cursor(prompt_enabled)(
                    active.before_cursor().as_ref(),
                    active.after_cursor().as_ref(),
                ),
                layout: Layout::Wrap {
                    line_breaker: BuiltInLineBreaker::AnyCharLineBreaker,
                    h_align: HorizontalAlign::Left,
                    v_align: VerticalAlign::Top,
                },
            });

            if !prompt_enabled {
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
    }

    /// Renders the currently active channel
    pub fn render_channel(&mut self, config: &SurfaceConfiguration) {
        let grammer = self.grammer.clone();

        if let (Some(glyph_brush), Some(active), Some(theme)) =
            self.prepare_render_output(self.channel as u32)
        {
            glyph_brush.queue(Section {
                screen_position: ((config.width as f32) / 2.0 + 60.0, 180.0),
                bounds: (config.width as f32 / 2.0, config.height as f32),
                text: theme.render(&grammer, active.output().as_ref(), false),
                layout: Layout::Wrap {
                    line_breaker: BuiltInLineBreaker::AnyCharLineBreaker,
                    h_align: HorizontalAlign::Left,
                    v_align: VerticalAlign::Top,
                },
            });
        }
    }
}

impl<G, Style> Extension for Shell<G, Style>
where
    Style: ColorTheme + Default,
    G: Grammer + Clone,
{
    fn configure_app_world(_world: &mut lifec::World) {
        _world.register::<ShellChannel>();

        _world.insert(wgpu::Color {
            r: 0.02122,
            g: 0.02519,
            b: 0.03434,
            a: 1.0,
        });
    }

    fn on_window_event(&'_ mut self, _app_world: &lifec::World, event: &'_ WindowEvent<'_>) {
        match (event, self.prepare_render_input()) {
            (WindowEvent::ReceivedCharacter(char), _) => {
                if let Some(sender) = &self.byte_tx {
                    if let Some(editing) = self.editing {
                        sender.try_send((editing as u32, *char as u8)).ok();
                    }
                }
            }
            (WindowEvent::KeyboardInput { input, .. }, (.., Some(editing), _theme)) => {
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
            }

            // TODO: This is a temp setting
            let mut default_context = ThunkContext::default();
            default_context
                .state_mut()
                .with_text("bracket", "purple")
                .with_text("operator", "yellow")
                .with_text("identifier", "red")
                .with_text("literal", "green")
                .with_text("comment", "green")
                .with_text("whitespace", "yellow");

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
        let mut send_to_connection = None;
        if let Some(rx) = self.byte_rx.as_mut() {
            if let Some((channel, next)) = rx.try_recv().ok() {
                if let Some(char_device) = self.char_devices.get_mut(&channel) {
                    if self.channel != channel as i32 && channel != 0 {
                        // TODO: Add this to a history
                        char_device.take_buffer();
                    }

                    char_device.write_char(next);
                    if char_device.line_count() > 1 && self.connection.is_some() && channel == 0 {
                        send_to_connection = Some(char_device.take_buffer());
                    }

                    self.channel = channel as i32;
                }
            }
        }

        if let Some(line) = send_to_connection.take() {
            if let Some(connection) = self.connection.take() {
                let tokio_runtime = app_world.read_resource::<tokio::runtime::Runtime>();
                let _ = tokio_runtime.enter();

                self.connection = tokio_runtime.block_on(async move {
                    event!(Level::TRACE, "Waiting for connection to be writeable");
                    connection.writable().await.ok();
                    // Line-endings need to be handled on the receiving end
                    let message = format!("{}\r\n", line);
                    match connection.try_write(message.as_bytes()) {
                        Ok(bytes) => {
                            event!(Level::TRACE, "Wrote {bytes}");
                            if bytes != message.len() {
                                event!(Level::WARN, "Did not write entire message");
                                todo!("Need to handle partialy sent messages")
                            }

                            Some(connection)
                        }
                        Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                            event!(Level::WARN, "Connection is not ready.");
                            Some(connection)
                        }
                        Err(err) => {
                            // Not actually hazardous but useful for posterity
                            event!(
                                Level::WARN,
                                "Connection to {} closed",
                                connection
                                    .local_addr()
                                    .ok()
                                    .and_then(|a| Some(a.to_string()))
                                    .unwrap_or_default()
                            );
                            event!(Level::DEBUG, "Error on connection close: {err}");
                            None
                        }
                    }
                });

                if self.connection.is_none() {
                    self.editing = None;
                }
            }
        }

        //  self.run_now(app_world);
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
                        theme.load_colors();

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

                if let Some(address) = self.address.as_mut() {
                    ui.input_text("address", address).build();

                    ui.same_line();
                    if ui.button("Connect to") {
                        if let Some(address) = self.address.clone() {
                            self.editing = Some(0);

                            let tokio_runtime =
                                app_world.read_resource::<tokio::runtime::Runtime>();
                            let _ = tokio_runtime.enter();

                            tokio_runtime.block_on(async move {
                                self.connect_to(address).await;
                            });
                        }
                    }
                }
            });
        });
    }
}

impl<'a, G, Style> System<'a> for Shell<G, Style>
where
    Style: ColorTheme + Default,
    G: Grammer + Clone,
{
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, ThunkContext>,
        WriteStorage<'a, ShellChannel>,
    );

    fn run(&mut self, (entities, mut contexts, mut channels): Self::SystemData) {
        for (entity, tc) in (&entities, &mut contexts).join() {
            if tc.is_enabled("enable_char_device") && !channels.contains(entity) {
                if let Some(channel) = self.add_device(entity) {
                    match channels.insert(entity, channel.clone()) {
                        Ok(_) => {
                            event!(Level::DEBUG, "Enabled char device for {:?}", entity);
                            tc.enable_output(channel.0.clone().unwrap());
                        }
                        Err(err) => {
                            event!(
                                Level::ERROR,
                                "Could not insert channel for {:?}, {err}",
                                entity
                            );
                        }
                    }
                }
            }
        }
    }
}
