#[derive(Debug, Default)]
pub struct ParserError {
    child: Option<std::io::Error>
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(error) = &self.child {
            write!(f, "Error encountered during parsing:\n{}", error)
        } else {
            write!(f, "Error encountered during parsing.")
        }
    }
}

impl std::error::Error for ParserError {}

impl From<std::io::Error> for ParserError {
    fn from(error: std::io::Error) -> Self {
        ParserError {
            child: Some(error)
        }
    }
}
