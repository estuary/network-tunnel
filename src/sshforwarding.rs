use std::any::Any;
use std::io::ErrorKind;
use std::process::Stdio;

use crate::errors::Error;
use crate::tunnel::NetworkTunnel;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::process::Command;

use serde::{Deserialize, Serialize};

pub const ENDPOINT_ADDRESS_KEY: &str = "address";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshForwardingConfig {
    /// Endpoint of the remote SSH server that supports tunneling, in the form of ssh://user@hostname[:port]
    pub ssh_endpoint: String,
    /// Private key to connect to the remote SSH server.
    pub private_key: String,
    /// The hostname of the remote destination (e.g. the database server).
    #[serde(default)]
    pub forward_host: String,
    /// The port of the remote destination (e.g. the database server).
    #[serde(default)]
    pub forward_port: u16,
    /// The local port which will be connected to the remote host/port over an SSH tunnel.
    /// This should match the port that's used in your basic connector configuration.
    #[serde(default)]
    pub local_port: u16,
}

pub struct SshForwarding {
    config: SshForwardingConfig,
    process: Option<Child>,
}

impl SshForwarding {
    pub fn new(config: SshForwardingConfig) -> Self {
        Self {
            config,
            process: None,
        }
    }
}

#[async_trait]
impl NetworkTunnel for SshForwarding {
    async fn prepare(&mut self) -> Result<(), Error> {
        let local_port = self.config.local_port;
        let ssh_endpoint = &self.config.ssh_endpoint;
        let forward_host = &self.config.forward_host;
        let forward_port = self.config.forward_port;

        tracing::info!(
            "ssh forwarding local port {} to remote host {}:{}",
            local_port,
            forward_host,
            forward_port
        );

        tracing::debug!("spawning ssh tunnel");
        let mut child = Command::new("ssh")
            .args(vec![
                // Disable psuedo-terminal allocation
                "-T".to_string(),
                // Be verbose so we can pick up signals about status of the tunnel
                "-v".to_string(),
                // This is necessary unless we also ask for the public key from users
                "-o".to_string(),
                "StrictHostKeyChecking no".to_string(),
                // Ask the client to time out after 5 seconds
                "-o".to_string(),
                "ConnectTimeout=5".to_string(),
                // Send period keepalive messages to the server to keep the
                // connection from being closed due to inactivity.
                "-o".to_string(),
                "ServerAliveInterval=30".to_string(),
                // Pass the private key
                "-i".to_string(),
                self.config.private_key.clone(),
                // Do not execute a remote command. Just forward the ports.
                "-N".to_string(),
                // Port forwarding stanza
                "-L".to_string(),
                format!("{local_port}:{forward_host}:{forward_port}"),
                ssh_endpoint.to_string(),
            ])
            .stderr(Stdio::piped())
            .spawn()?;

        // Read stderr of SSH until we find a signal message that
        // the ports are open and we are ready to serve requests
        let stderr = child.stderr.take().unwrap();
        let mut lines = BufReader::new(stderr).lines();
        self.process = Some(child);

        tracing::debug!("listening on ssh tunnel stderr");
        while let Some(line) = lines.next_line().await? {
            // OpenSSH will enter interactive session after tunnelling has been
            // successful
            if line.contains("Entering interactive session.") {
                tracing::debug!("ssh tunnel is listening & ready for serving requests");
                return Ok(());
            }

            // Otherwise apply a little bit of intelligence to translate OpenSSH
            // log messages to appropriate connector_proxy log levels.
            if line.starts_with("debug1:") {
                tracing::debug!("ssh: {}", &line);
            } else if line.starts_with("Warning: Permanently added") {
                tracing::debug!("ssh: {}", &line);
            } else if line.contains("Permission denied") {
                tracing::error!("ssh: {}", &line);
            } else if line.contains("Network is unreachable") {
                tracing::error!("ssh: {}", &line);
            } else if line.contains("Connection timed out") {
                tracing::error!("ssh: {}", &line);
            } else {
                tracing::info!("ssh: {}", &line);
            }
        }

        // This function's job was just to launch the SSH tunnel and wait until
        // it's ready to serve traffic. If stderr closes unexpectedly we treat
        // this as a probably-erroneous form of 'success', and rely on the later
        // `start_serve` exit code checking to report a failure.
        tracing::warn!("unexpected end of output from ssh tunnel");
        Ok(())
    }

    async fn start_serve(&mut self) -> Result<(), Error> {
        tracing::debug!("awaiting ssh tunnel process");
        let exit_status = self.process.as_mut().unwrap().wait().await?;
        if !exit_status.success() {
            tracing::error!(
                exit_code = ?exit_status.code(),
                message = "network tunnel ssh exit with non-zero code."
            );

            return Err(Error::TunnelExitNonZero(format!("{:#?}", exit_status)));
        }

        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Error> {
        if let Some(process) = self.process.as_mut() {
            match process.kill().await {
                // InvalidInput means the process has already exited, in which case
                // we do not need to cleanup the process
                Err(e) if e.kind() == ErrorKind::InvalidInput => Ok(()),
                a => a,
            }?;
        }

        Ok(())
    }

    // This is only used for testing
    fn as_any(&self) -> &dyn Any {
        self
    }
}
