use lifec::{App, System};
use lifec_shell::Shell;

fn main() {
    lifec::open(
        "basic example", 
        Empty{}, 
        Shell::default(),
    )
}

struct Empty; 

impl App for Empty {
    fn name() -> &'static str {
        "empty"
    }

    fn edit_ui(&mut self, ui: &imgui::Ui) {
    }

    fn display_ui(&self, ui: &imgui::Ui) {
    }
}

impl<'a> System<'a> for Empty {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
       
    }
}
