use pyo3::exceptions::PyException;
use pyo3::{PyErr, create_exception};

create_exception!(_internal, CaspersError, PyException);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Simulation error: {0}")]
    Simulation(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<Error> for PyErr {
    fn from(value: Error) -> Self {
        CaspersError::new_err(value.to_string())
    }
}
