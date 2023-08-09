#[derive(Debug)]
pub enum Error {
    EntryNotFound,
    // InvalidDatFile,
    IOError(std::io::Error),
    // SpriteTOCError,
    PNGEncodingError(png::EncodingError),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::IOError(e)
    }
}

impl From<png::EncodingError> for Error {
    fn from(e: png::EncodingError) -> Self {
        Self::PNGEncodingError(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::EntryNotFound => write!(f, "entry not found"),
            // Error::InvalidDatFile => write!(f, "invalid dat file"),
            Error::IOError(e) => write!(f, "{}", e),
            // Error::SpriteTOCError => write!(f, "error reading sprite toc"),
            Error::PNGEncodingError(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for Error {}
