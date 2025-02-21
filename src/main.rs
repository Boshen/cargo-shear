use std::process::ExitCode;

use cargo_shear::{CargoShear, cargo_shear_options};

fn main() -> ExitCode {
    let options = cargo_shear_options().run();
    CargoShear::new(options).run()
}
