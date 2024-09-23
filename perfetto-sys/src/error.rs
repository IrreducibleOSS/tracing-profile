use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error")]
    IOError(#[from] std::io::Error),
    #[error("external process error")]
    ProcessError(String, std::io::Error),
    #[error("external process {0} failed with code {1}")]
    ProcessReturnedError(String, i32),
}