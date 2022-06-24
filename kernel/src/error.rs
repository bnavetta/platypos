//! Standard kernel error type. Modules may use more specific errors if
//! appropriate.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// There was not enough memory to complete the operation.
    InsufficientMemory,
    /// The address was out of bounds (for example, it's outside of the current
    /// address space)
    AddressOutOfBounds,
}

impl Error {
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}
