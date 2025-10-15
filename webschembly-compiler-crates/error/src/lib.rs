#[derive(Debug, Clone)]
pub struct CompilerError(pub String);

impl std::fmt::Display for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilerError(message) => write!(f, "CompilerError: {}", message),
        }
    }
}

impl std::error::Error for CompilerError {}

#[macro_export]
macro_rules! compiler_error {
    ($($arg:tt)*) => {
        $crate::CompilerError(format!($($arg)*))
    }
}

pub type Result<T> = std::result::Result<T, CompilerError>;
