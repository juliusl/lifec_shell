use std::collections::BTreeMap;

use lifec::{Extension, Entity, Component, DenseVecStorage, WorldExt, Join};
use terminal_keycode::{Decoder, KeyCode};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{event, Level};
use wgpu::DepthStencilState;
use wgpu_glyph::{
    ab_glyph, BuiltInLineBreaker, GlyphBrush, GlyphBrushBuilder, HorizontalAlign, Layout, Section,
    Text, VerticalAlign,
};
use winit::event::ElementState;

/// Shell extension for the lifec runtime
#[derive(Default)]
pub struct Shell {
    /// glyph_brush, for rendering fonts
    brush: Option<GlyphBrush<DepthStencilState>>,
    /// terminal-keycode-decoder, use when connecting to remote processes
    decoder: Option<Decoder>,
    /// byte receiver
    byte_rx: Option<Receiver<(u32, u8)>>,
    /// byte sender
    byte_tx: Option<Sender<(u32, u8)>>,
    /// char-limit
    char_limit: usize,
    /// char-count
    char_count: usize,
    /// cursor
    cursor: usize,
    /// line number
    line: usize,
    /// char_devices
    char_devices: BTreeMap<u32, [u8; 1]>,
    /// character counts per line
    line_info: Vec<usize>,
    /// buffer
    buffer: Option<String>,
}

impl Shell {
    /// Returns the current line the cursor is on
    pub fn get_current_line(&self) -> Option<String> {
        self.get_line(self.line)
    }

    /// Returns the line at line_no
    pub fn get_line(&self, line_no: usize) -> Option<String> {
        if let Some(buffer) = &self.buffer {
            buffer
                .split('\r')
                .collect::<Vec<_>>()
                .get(line_no)
                .and_then(|l| Some(l.to_string()))
        } else {
            None
        }
    }

    pub fn goto_line(&mut self, line_no: usize) {
        let chars = self.line_info.iter().take(line_no + 1).sum::<usize>();

        self.cursor = chars + line_no;
    }

    /// Returns true if the shell was taken.
    pub fn add_device(&mut self, entity: Entity) -> Option<Sender<(u32, u8)>> {
        if let Some(tx) = self.byte_tx.clone() {
            let channel = entity.id();
            self.char_devices.insert(channel, [0; 1]);

            Some(tx)
        } else {
            None 
        }
    }
}

impl Extension for Shell {
    fn on_window_event(
        &'_ mut self,
        _app_world: &lifec::World,
        event: &'_ lifec::editor::WindowEvent<'_>,
    ) {
        match event {
            lifec::editor::WindowEvent::Resized(size) => {
                self.char_limit = (size.width / 16) as usize;
            }
            lifec::editor::WindowEvent::ReceivedCharacter(char) => {
                if let Some(sender) = &self.byte_tx {
                    sender.try_send((0, *char as u8)).ok();
                }
            }
            lifec::editor::WindowEvent::KeyboardInput { input, .. } => {
                match (input.virtual_keycode, input.state) {
                    (Some(key), ElementState::Released) => match key {
                        winit::event::VirtualKeyCode::Left => {
                            if self.cursor > 1
                                && !self.buffer.clone().unwrap_or_default().is_empty()
                            {
                                self.cursor -= 1;

                                if let Some(buffer) = &self.buffer {
                                    let check = self.cursor + 1;
                                    if let Some(b'\r') = buffer.as_bytes().get(check) {
                                        self.line -= 1;
                                    }
                                }
                            }
                        }
                        winit::event::VirtualKeyCode::Right => {
                            if self.cursor < self.buffer.clone().unwrap_or_default().len() {
                                self.cursor += 1;

                                if let Some(buffer) = &self.buffer {
                                    let check = self.cursor - 1;
                                    if let Some(b'\r') = buffer.as_bytes().get(check) {
                                        self.line += 1;
                                    }
                                }
                            }
                        }
                        winit::event::VirtualKeyCode::Down => {
                            if self.line < self.line_info.len() - 1 {
                                self.line += 1;
                                self.goto_line(self.line);
                            }
                        }
                        winit::event::VirtualKeyCode::Up => {
                            if self.line > 0 {
                                self.line -= 1;
                                self.goto_line(self.line);
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            lifec::editor::WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                self.char_limit = (new_inner_size.width / 16) as usize;
            }
            _ => {}
        }

        self.line_info = self
            .buffer
            .clone()
            .unwrap_or_default()
            .split('\r')
            .map(|l| l.len())
            .collect();

        if let Some(count) = self.line_info.get(self.line) {
            self.char_count = *count;
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
            self.decoder = Some(Decoder::new());

            let (tx, rx) = channel::<(u32, u8)>(300);
            self.byte_rx = Some(rx);
            self.byte_tx = Some(tx);
            self.buffer = Some(String::default());
            if self.char_devices.is_empty() {
                self.char_devices.insert(0, [0; 1]);
            }
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
        let current_line = &self.get_current_line();
        if let Self {
            brush: Some(glyph_brush),
            decoder: Some(decoder),
            byte_rx: Some(rx),
            byte_tx: _,
            char_limit,
            char_count,
            buffer: Some(buffer),
            char_devices,
            cursor,
            line,
            line_info,
        } = self
        {
            if let Some((channel, next)) = rx.try_recv().ok() {
                event!(Level::TRACE, "received {next} for char_device {channel}");
                if let Some(char_device) = char_devices.get_mut(&channel) {
                    char_device[0] = next;

                    // channel 0 is the input_buffer
                    if channel == 0 {
                        for keycode in decoder.write(next) {
                            if let Some(printable) = keycode.printable() {
                                buffer.insert(*cursor, printable);
                                *cursor += 1 as usize;
                            } else {
                                match keycode {
                                    KeyCode::Backspace => {
                                        if *cursor > 0 && !buffer.is_empty() {
                                            *cursor -= 1;
                                            match buffer.remove(*cursor) {
                                                '\r' | '\n' => {
                                                    if *line > 0 {
                                                        *line -= 1;
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
    
                            if keycode == KeyCode::Enter {
                                *line += 1;
                            }
                        }
                    }

                    // TODO, handle channels
                }
            }

            for (_, char_device) in self.char_devices.iter() {
                for keycode in decoder.write(char_device[0]) {
                    glyph_brush.queue(Section {
                        screen_position: (30.0, 30.0),
                        bounds: (config.width as f32, config.height as f32),
                        text: vec![Text::new(
                            format!(
                                "code={:?} bytes={:?} printable={:?}\rchar_limit={}\rchar_count={}\rcursor={} lines={} line_info={:?}\rcurrent_line={:?}",
                                keycode,
                                keycode.bytes(),
                                keycode.printable(),
                                char_limit,
                                char_count,
                                cursor,
                                line,
                                line_info,
                                current_line,
                            ).as_str(),
                        )
                        .with_color([1.0, 1.0, 1.0, 1.0])
                        .with_scale(40.0)],
                        ..Default::default()
                    });
                }
            }

            let cursor_tail = {
                if *cursor > 1 {
                    *cursor - 1
                } else {
                    0
                }
            };

            glyph_brush.queue(Section {
                screen_position: (30.0, 300.0),
                bounds: (config.width as f32, config.height as f32),
                text: {
                    vec![
                        Text::new("> ")
                            .with_color([1.0, 0.0, 0.0, 1.0])
                            .with_scale(40.0),
                        Text::new(&buffer)
                            .with_color([1.0, 1.0, 1.0, 1.0])
                            .with_scale(40.0)
                            .with_z(0.9),
                    ]
                },
                layout: Layout::Wrap {
                    line_breaker: BuiltInLineBreaker::AnyCharLineBreaker,
                    h_align: HorizontalAlign::Left,
                    v_align: VerticalAlign::Top,
                },
            });

            glyph_brush.queue(Section {
                screen_position: (30.0, 300.0),
                bounds: (config.width as f32, config.height as f32),
                text: {
                    vec![
                        Text::new("> ")
                            .with_color([1.0, 0.0, 0.0, 1.0])
                            .with_scale(40.0),
                        Text::new({
                            if !buffer.is_empty() {
                                &buffer[..*cursor]
                            } else {
                                ""
                            }
                        })
                        .with_color([1.0, 1.0, 1.0, 1.0])
                        .with_scale(40.0)
                        .with_z(-1.0),
                        Text::new("_")
                            .with_color([0.4, 0.8, 0.8, 1.0])
                            .with_scale(40.0)
                            .with_z(0.2),
                        Text::new({
                            if !buffer.is_empty() {
                                &buffer[cursor_tail..]
                            } else {
                                ""
                            }
                        })
                        .with_color([1.0, 1.0, 1.0, 1.0])
                        .with_scale(40.0)
                        .with_z(-1.0),
                    ]
                },
                layout: Layout::Wrap {
                    line_breaker: BuiltInLineBreaker::AnyCharLineBreaker,
                    h_align: HorizontalAlign::Left,
                    v_align: VerticalAlign::Top,
                },
            });

            // Draw the text!
            if let Some(depth_view) = depth_view.as_ref() {
                glyph_brush
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
        let mut shell_outputs = app_world.write_component::<ShellChannel>();
        let entities = app_world.entities();

        for (entity, shell_output) in (&entities, &mut shell_outputs).join() {
            if let ShellChannel(None) = shell_output {
                if let Some(tx) = self.add_device(entity) {
                    shell_output.0 = Some(tx);
                }
            }
        }
    }
}

/// This component adds a channel to this shell
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct ShellChannel(Option<Sender<(u32, u8)>>);