macro_rules! config {
    ($name:ident) => {
        #[derive(serde::Serialize, serde::Deserialize)]
        pub struct $name {
            #[serde(with = "humantime_serde")]
            pub timeout: std::time::Duration,
        }
    };
}

config!(Config);
