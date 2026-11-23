// Runs the tunnel against a real sshd to check that `--known-hosts` refuses a
// server whose host key isn't listed. Needs `/usr/sbin/sshd` and `ssh-keygen`.

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use network_tunnel::errors::Error;
use network_tunnel::sshforwarding::{SshForwarding, SshForwardingConfig};
use network_tunnel::tunnel::NetworkTunnel;

struct Sshd {
    dir: PathBuf,
    port: u16,
    process: Child,
}

impl Drop for Sshd {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn current_user() -> String {
    let output = Command::new("id").arg("-un").output().unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn keygen(path: &Path) {
    let status = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

/// Returns the key type and key of the public half of the key pair at `path`.
fn public_key(path: &Path) -> String {
    let contents = std::fs::read_to_string(path.with_extension("pub")).unwrap();
    contents
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}

fn start_sshd() -> Sshd {
    let dir = std::env::temp_dir().join(format!("network-tunnel-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    keygen(&dir.join("host_key"));
    keygen(&dir.join("client_key"));
    std::fs::copy(dir.join("client_key.pub"), dir.join("authorized_keys")).unwrap();

    // sshd runs as the test's user, so it can't use PAM or check file
    // ownership. MaxAuthTries leaves room for any keys in the user's ssh-agent,
    // which ssh offers before the tunnel's key.
    let port = free_port();
    let config = format!(
        "Port {port}
ListenAddress 127.0.0.1
HostKey {dir}/host_key
AuthorizedKeysFile {dir}/authorized_keys
PidFile none
UsePAM no
StrictModes no
PasswordAuthentication no
KbdInteractiveAuthentication no
MaxAuthTries 100
",
        dir = dir.display()
    );
    std::fs::write(dir.join("sshd_config"), config).unwrap();

    // sshd refuses to start unless it's run by its absolute path.
    let process = Command::new("/usr/sbin/sshd")
        .args(["-D", "-e", "-f"])
        .arg(dir.join("sshd_config"))
        .spawn()
        .expect("starting /usr/sbin/sshd");
    let sshd = Sshd { dir, port, process };

    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "sshd didn't start listening");
        std::thread::sleep(Duration::from_millis(100));
    }
    sshd
}

async fn prepare_with_known_hosts(sshd: &Sshd, known_hosts: &str) -> Result<(), Error> {
    let known_hosts_path = sshd.dir.join("known_hosts");
    std::fs::write(&known_hosts_path, known_hosts).unwrap();

    let mut tunnel = SshForwarding::new(SshForwardingConfig {
        ssh_endpoint: format!("ssh://{}@127.0.0.1:{}", current_user(), sshd.port),
        private_key: sshd.dir.join("client_key").display().to_string(),
        forward_host: "127.0.0.1".to_string(),
        forward_port: sshd.port,
        local_port: free_port(),
        known_hosts: Some(known_hosts_path.display().to_string()),
    });
    let result = tunnel.prepare().await;
    tunnel.cleanup().await.unwrap();
    result
}

#[tokio::test]
async fn test_known_hosts_against_sshd() {
    let sshd = start_sshd();
    let host = format!("[127.0.0.1]:{}", sshd.port);

    let listed = format!("{host} {}\n", public_key(&sshd.dir.join("host_key")));
    prepare_with_known_hosts(&sshd, &listed)
        .await
        .expect("the server's key is listed");

    keygen(&sshd.dir.join("other_key"));
    let wrong = format!("{host} {}\n", public_key(&sshd.dir.join("other_key")));
    match prepare_with_known_hosts(&sshd, &wrong).await {
        Err(Error::SSH(message)) => assert!(
            message.contains("has changed and you have requested strict checking"),
            "{message}"
        ),
        other => panic!("expected a host key error, got {other:?}"),
    }

    match prepare_with_known_hosts(&sshd, "").await {
        Err(Error::SSH(message)) => assert!(
            message.contains("host key is known for") && message.contains("strict checking"),
            "{message}"
        ),
        other => panic!("expected a host key error, got {other:?}"),
    }
}
