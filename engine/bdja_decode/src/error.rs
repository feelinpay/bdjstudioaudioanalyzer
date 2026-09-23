use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Error de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("Archivo vacio o de longitud cero")]
    ZeroLength,

    #[error("Archivo excede el limite maximo de 2 GB ({0} bytes)")]
    FileTooLarge(u64),

    #[error("Duracion excede el limite maximo de 3 horas ({0} ms)")]
    DurationExceeded(u64),

    #[error("Formato o códec no soportado por el motor: {0}")]
    Unsupported(String),

    #[error("Formato o contenedor no reconocido por el motor: {0}")]
    UnrecognizedFormat(String),

    #[error("Cabecera o flujo de audio corrupto: {0}")]
    CorruptedHeader(String),

    #[error("No se encontraron pistas de audio en el archivo")]
    NoAudioTrack,

    #[error("Error al instanciar el decodificador: {0}")]
    DecoderInit(String),

    #[error("Error durante la decodificacion de paquetes: {0}")]
    PacketDecode(String),

    #[error("Error de Symphonia: {0}")]
    Symphonia(String),
}

pub type Result<T> = std::result::Result<T, DecodeError>;
