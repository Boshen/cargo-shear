use cargo_shear::shear;

use cargo_shear::options;

fn main() {
    let options = options().fallback_to_usage().run();
    shear(&options);
}
