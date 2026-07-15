use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
}
