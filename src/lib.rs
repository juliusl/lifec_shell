use lifec::Extension;
use std::fmt::Write;
use terminal_keycode::{Decoder, KeyCode};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use wgpu_glyph::{ab_glyph, GlyphBrush, GlyphBrushBuilder, Section, Text};

#[derive(Default)]
pub struct Shell(
    /// glyph_brush, for rendering fonts
    Option<GlyphBrush<()>>,
    /// terminal-keycode-decoder, use when connecting to remote processes
    Option<Decoder>,
    /// byte receiver
    Option<Receiver<u8>>,
    /// byte sender
    Option<Sender<u8>>,
    /// char-limit
    u32,
    /// char-count
    u32,
    /// line
    Option<String>,
    /// buffer
    [u8; 1],
);

impl Extension for Shell {
    fn on_window_event(
        &'_ mut self,
        _app_world: &lifec::World,
        event: &'_ lifec::editor::WindowEvent<'_>,
    ) {
        match event {
            lifec::editor::WindowEvent::Resized(size) => {
                self.4 = size.width / 16;
            }
            lifec::editor::WindowEvent::ReceivedCharacter(char) => {
                if let Some(sender) = &self.3 {
                    sender.try_send(*char as u8).ok();
                }
            }
            lifec::editor::WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                self.4 = new_inner_size.width / 16;
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

            self.0 = Some(glyph_brush);
            self.1 = Some(Decoder::new());

            let (tx, rx) = channel::<u8>(100);
            self.2 = Some(rx);
            self.3 = Some(tx);
            self.6 = Some(String::default());
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
        if let Self(
            Some(glyph_brush),
            Some(decoder),
            Some(rx),
            Some(tx),
            char_limit,
            mut char_count,
            Some(line),
            buf,
        ) = self
        {
            if let Some(next) = rx.try_recv().ok() {
                char_count += 1;
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
                                if char_count > 1 {
                                    char_count -= 2;
                                }
                            }
                            _ => {}
                        }
                    }

                    if keycode == KeyCode::Enter {
                        char_count = 0;
                    }
                }

                if char_count > *char_limit {
                    tx.try_send('\n' as u8).ok();
                    char_count = 0;
                }
            }

            for keycode in decoder.write(buf[0]) {
                glyph_brush.queue(Section {
                    screen_position: (30.0, 30.0),
                    bounds: (config.width as f32, config.height as f32),
                    text: vec![Text::new(
                        format!(
                            "code={:?} bytes={:?} printable={:?}\rchar_limit={}\rchar_count={}",
                            keycode,
                            keycode.bytes(),
                            keycode.printable(),
                            char_limit,
                            char_count,
                        )
                        .as_str(),
                    )
                    .with_color([1.0, 1.0, 1.0, 1.0])
                    .with_scale(40.0)],
                    ..Section::default()
                });
            }

            glyph_brush.queue(Section {
                screen_position: (30.0, 150.0),
                bounds: (config.width as f32, config.height as f32),
                text: {
                    vec![
                        Text::new("> ")
                            .with_color([1.0, 0.0, 0.0, 1.0])
                            .with_scale(40.0),
                        Text::new(line.as_str())
                            .with_color([1.0, 1.0, 1.0, 1.0])
                            .with_scale(40.0),
                        Text::new("_")
                            .with_color([0.4, 0.8, 0.8, 1.0])
                            .with_scale(40.0),
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
