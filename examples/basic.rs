use lifec::{combine_default, App, System, editor::RuntimeEditor};
use lifec_shell::Shell;

fn main() {
    lifec::open(
        "basic example", 
        Empty{}, 
        combine_default::<RuntimeEditor, Shell>(),
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
