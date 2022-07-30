use lifec::{combine_default, App, System, editor::RuntimeEditor, plugins::{Config, Plugin, Process, Remote}};
use lifec_shell::Shell;

fn main() {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .compact()
        .init();

    let mut extension = combine_default::<RuntimeEditor, Shell>();

    extension.1.runtime_editor_mut().runtime_mut().add_config(Config("shell", |a| {
        a.block.block_name = a.label("new_remote").as_ref().to_string();
        a.as_mut()
        .with_text("node_title", "Remote sh")
        .with_text("thunk_symbol", Remote::symbol())
        .with_bool("default_open", true)
        .with_bool("enable_listener", true)
        .with_text("command", "zsh");
    }));

    lifec::open(
        "basic example", 
        Empty{}, 
        extension,
    )
}

struct Empty; 

impl App for Empty {
    fn name() -> &'static str {
        "empty"
    }

    fn enable_depth_stencil<'a>(&self) -> bool {
        true
    }

    fn edit_ui(&mut self, _ui: &imgui::Ui) {
    }

    fn display_ui(&self, _ui: &imgui::Ui) {
    }
}

impl<'a> System<'a> for Empty {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
       
    }
}
