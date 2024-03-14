use std::{env, path::Path};

pub fn shear() {
    let path = env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let path = Path::new(&path);
    dbg!(path);
}
