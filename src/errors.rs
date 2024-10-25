#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("SSH forwarding network tunnel exit with non-zero exit code {0}")]
    TunnelExitNonZero(String),

    // Used to bubble up SSH tunnel errors without logging any further errors
    // this allows the last `ssh: ` log to be reported as the main error to the user
    #[error("{0}")]
    SSH(String)
}
