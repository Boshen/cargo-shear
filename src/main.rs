#[cfg(feature = "allocator")]
#[global_allocator]
static GLOBAL: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

use std::process::ExitCode;

use cargo_shear::{CargoShear, cargo_shear_options};

fn main() -> ExitCode {
    let options = cargo_shear_options().run();
    CargoShear::new(std::io::stdout(), options).run()
}
