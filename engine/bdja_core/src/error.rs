use thiserror::Error;

#[derive(Error, Debug)]
pub enum BdjaError {
    #[error("Capacidad no autorizada: token de licencia invalido o expirado")]
    UnauthorizedCapability,

    #[error("Error de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("Error de decodificacion: {0}")]
    Decode(String),

    #[error("Error en base de datos SQLite: {0}")]
    Database(String),

    #[error("Error de IPC: {0}")]
    Ipc(String),

    #[error("El archivo excede los limites seguros (max 2 GB, max 3 horas): {0}")]
    LimitsExceeded(String),

    #[error("Operacion cancelada por el usuario")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, BdjaError>;
