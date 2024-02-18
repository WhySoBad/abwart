use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Tag {
    pub name: String,
    pub digest: String,
    pub created: DateTime<Utc>,
    pub size: u64
}

impl Tag {
    pub fn new(name: String, digest: String, created: DateTime<Utc>, size: u64) -> Self {
        Self { name, digest, created, size }
    }
}