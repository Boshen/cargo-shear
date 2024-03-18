use std::process::ExitCode;

use cargo_shear::{cargo_shear_options, CargoShear};

fn main() -> ExitCode {
    let options = cargo_shear_options().fallback_to_usage().run();
    CargoShear::new(options).run()
}
