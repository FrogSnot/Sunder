use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("audio: {0}")]
    Audio(String),
    #[error("database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("extraction: {0}")]
    Extraction(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
