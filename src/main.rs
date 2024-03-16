use cargo_shear::{cargo_shear_options, CargoShear};

fn main() {
    let options = cargo_shear_options().fallback_to_usage().run();
    CargoShear::new(options).run();
}
