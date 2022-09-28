use std::any::Any;

use crate::errors::Error;

use async_trait::async_trait;

#[async_trait]
pub trait NetworkTunnel: Send + Sync {
    // Setup the network proxy server. Network proxy should be able to listen and accept requests after `prepare` is performed.
    async fn prepare(&mut self) -> Result<(), Error>;
    // Start a long-running task that serves and processes all proxy requests from clients.
    async fn start_serve(&mut self) -> Result<(), Error>;
    // Cleanup the child process. This is called in cases of failure to make sure the child process
    // is properly killed.
    async fn cleanup(&mut self) -> Result<(), Error>;

    // This is only used for testing purposes
    fn as_any(&self) -> &dyn Any;
}
