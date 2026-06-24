use crate::error::ServerResult;

pub struct Config {
    pub port: u16,
    pub static_dir: String,
}

impl Config {
    pub fn from_env() -> ServerResult<Self> {
        Ok(Self {
            port: std::env::var("PORT")
                .unwrap_or("61330".to_string())
                .parse::<u16>()?,
            static_dir: std::env::var("STATIC_DIR").unwrap_or("./static".to_string()),
        })
    }
}
