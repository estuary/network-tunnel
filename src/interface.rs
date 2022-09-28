use super::networktunnel::NetworkTunnel;
use super::sshforwarding::{SshForwarding, SshForwardingConfig};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum NetworkTunnelConfig {
    SshForwarding(SshForwardingConfig),
}
