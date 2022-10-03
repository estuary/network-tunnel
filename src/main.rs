use clap::Parser;
use network_tunnel::errors::Error;
use flow_cli_common::{init_logging, LogArgs};
use futures::future::{self, TryFutureExt};
use network_tunnel::{sshforwarding::{SshForwarding, SshForwardingConfig}, tunnel::NetworkTunnel};
use std::io;

#[derive(clap::Subcommand, Clone, Debug)]
pub enum Command {
    SSH {
        /// Endpoint of the remote SSH server that supports tunneling, in the form of ssh://user@hostname[:port]
        #[clap(long)]
        ssh_endpoint: String,

        #[clap(long)]
        /// Path to private key file to connect to the remote SSH server. The file must have
        /// permissions recommended by SSH (http://linuxcommand.org/lc3_man_pages/ssh1.html).
        /// Recommended permissions: 600.
        private_key: String,

        /// The hostname of the remote destination (e.g. the database server).
        #[clap(long)]
        forward_host: String,

        /// The port of the remote destination (e.g. the database server).
        #[clap(long)]
        forward_port: u16,

        /// The local port which will be connected to the remote host/port over an SSH tunnel.
        /// This should match the port that's used in your connector configuration.
        #[clap(long)]
        local_port: u16,
    }
}

#[derive(Parser, Debug)]
#[clap(author, version, about = "Start a network tunnel and port-forward specific ports on the destination host through the tunnel.")]
pub struct Args {
    /// The command used to run the underlying airbyte connector
    #[clap(subcommand)]
    command: Command,

    #[clap(flatten)]
    log_args: LogArgs,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let Args {
        command,
        log_args,
    } = Args::parse();

    init_logging(&log_args);

    if let Err(err) = run(command).await.as_ref() {
        tracing::error!(error = ?err, "network tunnel failed.");
        std::process::exit(1);
    }
    Ok(())
}

async fn run_and_cleanup(tunnel: &mut Box<dyn NetworkTunnel>) -> Result<(), Error> {
    let tunnel_block = {
        let prep = tunnel.prepare().await;

        // Write "READY" to stdio to unblock Go logic.
        // The current workflow assumes that
        //   1. After tunnel.prepare() is called, the network tunnel is able to accept requests from clients without sending errors back to clients.
        //   2. The network tunnel is able to process client requests immediately after `tunnel.start_serve` is called.
        // If either of the assumptions is invalid for any new tunnel type, the READY-logic need to be moved to a separate task, which
        //    sends out the "READY" signal after making sure the network tunnel is started and working properly.
        println!("READY");

        future::ready(prep).and_then(|()| {
            tunnel.start_serve()
        }).await
    };

    // We must make sure we cleanup the child process. This is specially important
    // as processes that are not `wait`ed on can end up as zombies in some operating
    // systems (see https://doc.rust-lang.org/std/process/struct.Child.html#warning)
    tunnel.cleanup().await?;

    tunnel_block
}

async fn run(cmd: Command) -> Result<(), Error> {
    match cmd {
        Command::SSH { ssh_endpoint, private_key, forward_host, forward_port, local_port } => {
            let mut tunnel: Box<dyn NetworkTunnel> = Box::new(SshForwarding::new(
                SshForwardingConfig {
                    ssh_endpoint,
                    private_key,
                    forward_host,
                    forward_port,
                    local_port,
                }
            ));

            run_and_cleanup(&mut tunnel).await
        }
    }
}

#[cfg(test)]
mod test {
    use std::any::Any;

    use async_trait::async_trait;
    use network_tunnel::errors::Error;
    use network_tunnel::tunnel::NetworkTunnel;

    use crate::run_and_cleanup;

    #[derive(Debug)]
    struct TestTunnel {
        cleanup_called: bool,
        error_in_prepare: bool,
        error_in_serve: bool,
    }

    #[async_trait]
    impl NetworkTunnel for TestTunnel {
        async fn prepare(&mut self) -> Result<(), Error> {
            if self.error_in_prepare {
                return Err(Error::TunnelExitNonZero("prepare-error".to_string()))
            }

            Ok(())
        }

        async fn start_serve(&mut self) -> Result<(), Error> {
            if self.error_in_serve {
                return Err(Error::TunnelExitNonZero("serve-error".to_string()))
            }

            Ok(())
        }

        async fn cleanup(&mut self) -> Result<(), Error> {
            self.cleanup_called = true;

            Ok(())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }


    #[tokio::test]
    async fn test_cleanup_call_error_in_prepare() {
        let mut tunnel : Box<dyn NetworkTunnel> = Box::new(TestTunnel {
            cleanup_called: false,
            error_in_prepare: true,
            error_in_serve: false,
        });

        let result = run_and_cleanup(&mut tunnel).await;
        assert!(result.is_err());

        let test_tunnel = tunnel.as_any().downcast_ref::<TestTunnel>().unwrap();
        assert!(test_tunnel.cleanup_called);
    }

    #[tokio::test]
    async fn test_cleanup_call_error_in_serve() {
        let mut tunnel : Box<dyn NetworkTunnel> = Box::new(TestTunnel {
            cleanup_called: false,
            error_in_prepare: false,
            error_in_serve: true,
        });

        let result = run_and_cleanup(&mut tunnel).await;
        assert!(result.is_err());

        let test_tunnel = tunnel.as_any().downcast_ref::<TestTunnel>().unwrap();
        assert!(test_tunnel.cleanup_called);
    }

    #[tokio::test]
    async fn test_cleanup_call_success() {
        let mut tunnel : Box<dyn NetworkTunnel> = Box::new(TestTunnel {
            cleanup_called: false,
            error_in_prepare: false,
            error_in_serve: false,
        });

        let result = run_and_cleanup(&mut tunnel).await;
        assert!(result.is_ok());

        let test_tunnel = tunnel.as_any().downcast_ref::<TestTunnel>().unwrap();
        assert!(test_tunnel.cleanup_called);
    }
}
