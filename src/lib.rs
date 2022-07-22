use lifec::Extension;
use std::fmt::Write;
use terminal_keycode::{Decoder, KeyCode};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use wgpu_glyph::{ab_glyph, GlyphBrush, GlyphBrushBuilder, Section, Text};

#[derive(Default)]
pub struct Shell {
    /// glyph_brush, for rendering fonts
    brush: Option<GlyphBrush<()>>,
    /// terminal-keycode-decoder, use when connecting to remote processes
    decoder: Option<Decoder>,
    /// byte receiver
    byte_rx: Option<Receiver<u8>>,
    /// byte sender
    byte_tx: Option<Sender<u8>>,
    /// char-limit
    char_limit: u32,
    /// char-count
    char_count: u32,
    /// line
    line: Option<String>,
    /// buffer
    buf: [u8; 1],
    /// cursor
    cursor: usize,
}

impl Extension for Shell {
    fn on_window_event(
        &'_ mut self,
        _app_world: &lifec::World,
        event: &'_ lifec::editor::WindowEvent<'_>,
    ) {
        match event {
            lifec::editor::WindowEvent::Resized(size) => {
                self.char_limit = size.width / 16;
            }
            lifec::editor::WindowEvent::ReceivedCharacter(char) => {
                if let Some(sender) = &self.byte_tx {
                    sender.try_send(*char as u8).ok();
                }
            }
            lifec::editor::WindowEvent::KeyboardInput { input, .. } => {
                match input.virtual_keycode {
                    Some(key) => {
                        match key {
                            winit::event::VirtualKeyCode::Left => {
                                if self.cursor > 1 {
                                    self.cursor -= 1;
                                }
                            },
                            winit::event::VirtualKeyCode::Right => {
                                if self.cursor < self.line.clone().unwrap_or_default().len() {
                                    self.cursor += 1;
                                }
                            },
                            winit::event::VirtualKeyCode::Down => {},
                            winit::event::VirtualKeyCode::Up => {},
                            _ => {}
                        }
                    },
                    _ => {},
                }
            }
            lifec::editor::WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                self.char_limit = new_inner_size.width / 16;
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
                .build(&device, wgpu::TextureFormat::Bgra8UnormSrgb);

            self.brush = Some(glyph_brush);
            self.decoder = Some(Decoder::new());

            let (tx, rx) = channel::<u8>(100);
            self.byte_rx = Some(rx);
            self.byte_tx = Some(tx);
            self.line = Some(String::default());
        }
    }

    fn on_render(
        &'_ mut self,
        view: &wgpu::TextureView,
        _surface: &wgpu::Surface,
        config: &wgpu::SurfaceConfiguration,
        _adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        staging_belt: &mut wgpu::util::StagingBelt,
    ) {
        if let Self {
            brush: Some(glyph_brush),
            decoder: Some(decoder),
            byte_rx: Some(rx),
            byte_tx: Some(tx),
            char_limit,
            char_count,
            line: Some(line),
            buf,
            cursor,
         } = self
        {
            if let Some(next) = rx.try_recv().ok() {
                *char_count += 1;
                *cursor += 1 as usize;
                buf[0] = next;

                for keycode in decoder.write(next) {
                    if let Some(printable) = keycode.printable() {
                        match write!(line, "{}", printable) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    } else {
                        match keycode {
                            KeyCode::Backspace => {
                                line.pop();
                                if *char_count > 1 {
                                    *char_count -= 2;
                                }

                                if *cursor > 1 {
                                    *cursor -= 2;
                                }
                            }
                            _ => {}
                        }
                    }

                    if keycode == KeyCode::Enter {
                        *char_count = 0;
                    }
                }

                if *char_count > *char_limit {
                    tx.try_send('\n' as u8).ok();
                    *char_count = 0;
                }
            }

            for keycode in decoder.write(buf[0]) {
                glyph_brush.queue(Section {
                    screen_position: (30.0, 30.0),
                    bounds: (config.width as f32, config.height as f32),
                    text: vec![Text::new(
                        format!(
                            "code={:?} bytes={:?} printable={:?}\rchar_limit={}\rchar_count={}\rcursor={}",
                            keycode,
                            keycode.bytes(),
                            keycode.printable(),
                            char_limit,
                            char_count,
                            cursor,
                        )
                        .as_str(),
                    )
                    .with_color([1.0, 1.0, 1.0, 1.0])
                    .with_scale(40.0)],
                    ..Section::default()
                });
            }
            
            glyph_brush.queue(Section {
                screen_position: (30.0, 180.0),
                bounds: (config.width as f32, config.height as f32),
                text: {
                    vec![
                        Text::new("> ")
                            .with_color([1.0, 0.0, 0.0, 1.0])
                            .with_scale(40.0),
                        Text::new(&line[..*cursor])
                            .with_color([1.0, 1.0, 1.0, 1.0])
                            .with_scale(40.0),
                        Text::new("_")
                            .with_color([0.4, 0.8, 0.8, 1.0])
                            .with_scale(40.0)
                            .with_z(0.8),
                        Text::new(&line[*cursor..])
                            .with_color([1.0, 1.0, 1.0, 1.0])
                            .with_scale(40.0)
                    ]
                },
                ..Section::default()
            });

            // Draw the text!
            glyph_brush
                .draw_queued(
                    device,
                    staging_belt,
                    encoder,
                    view,
                    config.width,
                    config.height,
                )
                .expect("Draw queued");
        }
    }
}
