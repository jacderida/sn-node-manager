mod control;
mod install;
mod node;
mod service;

use crate::control::start;
use crate::install::{get_node_registry_path, install};
use crate::node::NodeRegistry;
use crate::service::NodeServiceManager;
use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Help, Result};
use libp2p_identity::PeerId;
use sn_releases::SafeReleaseRepositoryInterface;
use sn_rpc_client::RpcClient;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cmd {
    /// Available sub commands.
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Subcommand, Debug)]
pub enum SubCmd {
    /// Install safenode as a service.
    ///
    /// This command must run as the root/administrative user.
    #[clap(name = "install")]
    Install {
        /// The number of service instances
        #[clap(long)]
        count: Option<u16>,
        /// The user the service should run as.
        ///
        /// If the account does not exist, it will be created.
        ///
        /// On Windows this argument will have no effect.
        #[clap(long)]
        user: Option<String>,
        /// The version of safenode
        #[clap(long)]
        version: Option<String>,
    },
    /// Start an installed safenode service.
    ///
    /// If no peer ID(s) or service name(s) are supplied, all installed services will be started.
    #[clap(name = "start")]
    Start {
        /// The peer ID of the service to start.
        #[clap(long)]
        peer_id: Option<String>,
        /// The name of the service to start.
        #[clap(long)]
        service_name: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cmd::parse();
    match args.cmd {
        SubCmd::Install {
            count,
            user,
            version,
        } => {
            if !is_running_as_root() {
                return Err(eyre!("The install command must run as the root user"));
            }
            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            let release_repo = <dyn SafeReleaseRepositoryInterface>::default_config();
            install(
                get_safenode_install_path()?,
                count,
                user,
                version,
                &mut node_registry,
                &NodeServiceManager {},
                release_repo,
            )
            .await?;
            node_registry.save(&get_node_registry_path()?)?;
            Ok(())
        }
        SubCmd::Start {
            peer_id,
            service_name,
        } => {
            let mut node_registry = NodeRegistry::load(&get_node_registry_path()?)?;
            if service_name.is_some() && peer_id.is_some() {
                return Err(eyre!("The service name and peer ID are mutually exclusive")
                    .suggestion(
                    "Please try again using either the peer ID or the service name, but not both.",
                ));
            }

            if let Some(ref name) = service_name {
                let mut node = node_registry
                    .installed_nodes
                    .iter_mut()
                    .find(|x| x.service_name == *name)
                    .ok_or_else(|| eyre!("No service named '{name}'"))?;

                let rpc_client = RpcClient::new(&format!("127.0.0.1:{}", node.rpc_port));
                start(&mut node, &NodeServiceManager {}, &rpc_client).await?;
            } else if let Some(ref peer_id) = peer_id {
                let peer_id = PeerId::from_str(&peer_id)?;
                let mut node = node_registry
                    .installed_nodes
                    .iter_mut()
                    .find(|x| x.peer_id == Some(peer_id))
                    .ok_or_else(|| {
                        eyre!(format!(
                            "Could not find node with peer ID '{}'",
                            peer_id.to_string()
                        ))
                    })?;

                let rpc_client = RpcClient::new(&format!("127.0.0.1:{}", node.rpc_port));
                start(&mut node, &NodeServiceManager {}, &rpc_client).await?;
            } else {
                for mut node in node_registry.installed_nodes.iter_mut() {
                    let rpc_client = RpcClient::new(&format!("127.0.0.1:{}", node.rpc_port));
                    start(&mut node, &NodeServiceManager {}, &rpc_client).await?;
                }
            }
            Ok(())
        }
    }
}

#[cfg(unix)]
fn is_running_as_root() -> bool {
    users::get_effective_uid() == 0
}

#[cfg(windows)]
fn is_running_as_root() -> bool {
    // The Windows implementation for this will be much more complex.
    true
}

#[cfg(unix)]
fn get_safenode_install_path() -> Result<PathBuf> {
    Ok(PathBuf::from("/usr/local/bin"))
}

#[cfg(windows)]
fn get_safenode_install_path() -> Result<PathBuf> {
    let path = PathBuf::from("C:\\Program Files\\Maidsafe\\safenode");
    if !path.exists() {
        std::fs::create_dir_all(path.clone())?;
    }
    Ok(path)
}