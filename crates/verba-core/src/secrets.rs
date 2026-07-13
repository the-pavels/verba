#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretStoreError {
    InvalidSecret,
    Corrupted,
    Unavailable,
}

pub trait SecretStore: Send + Sync {
    fn save(&self, secret: &str) -> Result<(), SecretStoreError>;

    fn load(&self) -> Result<Option<String>, SecretStoreError>;

    fn delete(&self) -> Result<(), SecretStoreError>;
}
