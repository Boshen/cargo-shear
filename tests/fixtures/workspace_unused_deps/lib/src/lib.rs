use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Data {
    pub value: String,
}

pub fn get_data() -> String {
    let data = Data {
        value: "Hello from lib".to_string(),
    };
    format!("Data: {}", data.value)
}