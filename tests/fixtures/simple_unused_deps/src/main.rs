use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Config {
    name: String,
    value: i32,
}

fn main() {
    let config = Config {
        name: "test".to_string(),
        value: 42,
    };
    
    println!("Config: {} = {}", config.name, config.value);
}