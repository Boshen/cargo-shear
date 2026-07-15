pub struct Config {
    #[serde(with = "humantime_serde")]
    pub timeout: std::time::Duration,
}
