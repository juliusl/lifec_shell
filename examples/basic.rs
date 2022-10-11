use lifec::{combine_default, App, System};
use lifec_shell::Shell;

fn main() {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .compact()
        .init();

    // let extension = combine_default::<RuntimeEditor, Shell>();

    // lifec::open(
    //     "basic example", 
    //     Empty{}, 
    //     extension,
    // )
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
