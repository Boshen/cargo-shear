use serde::{Serialize, Deserialize};
use uuid::Uuid;
use rand::Rng;

#[derive(Serialize, Deserialize)]
pub struct Entity {
    pub id: Uuid,
    pub name: String,
}

impl Entity {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
        }
    }
    
    pub fn random_number() -> u32 {
        let mut rng = rand::thread_rng();
        rng.gen()
    }
}