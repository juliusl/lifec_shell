use lifec::{Component, DenseVecStorage, Entity, Extension, Join, WorldExt};
use std::collections::BTreeMap;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{event, Level};
use wgpu::{DepthStencilState, SurfaceConfiguration};
use wgpu_glyph::{
    ab_glyph, BuiltInLineBreaker, GlyphBrush, GlyphBrushBuilder, HorizontalAlign, Layout, Section,
    Text, VerticalAlign,
};
use winit::event::ElementState;

mod char_device;
pub use char_device::CharDevice;

/// Shell extension for the lifec runtime
#[derive(Default)]
pub struct Shell<Theme = DefaultTheme> {
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
    theme: Theme,
}

#[derive(Default)]
pub struct DefaultTheme;

impl InputTheme for DefaultTheme {
    fn prompt(&self) -> Text<'_> {
        Text::new("> ")
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_scale(40.0)
    }

    fn cursor(&self) -> Text<'_> {
        Text::new("_")
             .with_color([0.4, 0.8, 0.8, 1.0])
             .with_scale(40.0)
             .with_z(0.2)
    }
}

/// This component adds a channel to this shell
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct ShellChannel(Option<Sender<(u32, u8)>>);

/// Trait to edit parts of the shell
pub trait InputTheme {
    fn prompt(&self) -> Text<'_,>;
    fn cursor(&self) -> Text<'_>;
}

impl Shell {
    /// Returns the text brush and char device being edited
    pub fn prepare_render_input(
        &mut self,
    ) -> (
        Text,
        Text,
        Option<&mut GlyphBrush<DepthStencilState>>,
        Option<&mut CharDevice>,
    ) {
        if let Some(editing) = self.editing {
            (self.theme.prompt(), self.theme.cursor(), self.brush.as_mut(), self.char_devices.get_mut(&editing))
        } else {
            (self.theme.prompt(), self.theme.cursor(), None, None)
        }
    }

    /// Returns true if the shell was taken.
    pub fn add_device(&mut self, entity: Entity) -> Option<ShellChannel> {
        if let Some(tx) = self.byte_tx.clone() {
            let channel = entity.id();
            self.char_devices.insert(channel, CharDevice::default());

            Some(ShellChannel(Some(tx)))
        } else {
            None
        }
    }

    /// Renders the input section
    pub fn render_input(&mut self, config: &SurfaceConfiguration) {
        if let (prompt, cursor, Some(glyph_brush), Some(active)) = self.prepare_render_input() {
            
            // Renders the buffer
            glyph_brush.queue(Section {
                screen_position: (30.0, 300.0),
                bounds: (config.width as f32, config.height as f32),
                text: {
                    vec![
                        prompt,
                        Text::new(active.output().as_ref())
                            .with_color([0.8, 0.4, 0.3, 1.0])
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

            // Renders the cursor
            glyph_brush.queue(Section {
                screen_position: (30.0, 300.0),
                bounds: (config.width as f32, config.height as f32),
                text: {
                    vec![
                        prompt,
                        Text::new(active.before_cursor().as_ref())
                            .with_color([0.0, 0.0, 0.0, 0.0])
                            .with_scale(40.0)
                            .with_z(1.0),
                        cursor,
                        Text::new(active.after_cursor().as_ref())
                            .with_color([0.0, 0.0, 0.0, 0.0])
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
        }
    }
}

impl Extension for Shell {
    fn configure_app_world(_world: &mut lifec::World) {
        _world.register::<ShellChannel>();
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
            (lifec::editor::WindowEvent::KeyboardInput { input, .. }, (.., Some(editing))) => {
                match (input.virtual_keycode, input.state) {
                    (Some(key), ElementState::Released) => match key {
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
        let entities = app_world.entities();

        for (entity, shell_output) in (&entities, &mut shell_outputs).join() {
            if let ShellChannel(None) = shell_output {
                if let Some(channel) = self.add_device(entity) {
                    *shell_output = channel;
                }
            }
        }
    }
}

// for (_, CharDevice { write_buffer: char_device, decoder, buffer: output, .. }) in self.char_devices.iter_mut() {
//     for keycode in decoder.write(char_device[0]) {
//         glyph_brush.queue(Section {
//             screen_position: (30.0, 30.0),
//             bounds: (config.width as f32, config.height as f32),
//             text: vec![Text::new(
//                 format!(
//                     "code={:?} bytes={:?} printable={:?}",
//                     keycode,
//                     keycode.bytes(),
//                     keycode.printable(),
//                 ).as_str(),
//             )
//             .with_color([1.0, 1.0, 1.0, 1.0])
//             .with_scale(40.0)],
//             ..Default::default()
//         });
//     }

//     glyph_brush.queue(Section {
//         screen_position: (200.0, 30.0),
//         bounds: (config.width as f32, config.height as f32),
//         text: vec![Text::new(output.as_str())
//         .with_color([1.0, 1.0, 1.0, 1.0])
//         .with_scale(40.0)],
//         ..Default::default()
//     });
// }
