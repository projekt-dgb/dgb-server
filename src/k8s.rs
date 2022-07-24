//! Nachdem Änderungen validiert wurden, müssen diese auf alle Nodes
//! im Cluster verteilt werden, damit andere Nodes von den Änderungen
//! etwas mitbekommen.
//!
//! Das heißt, bei jeder Änderung, die Daten schreibt, müssen die k8s-interne IPs
//! ausgelesen werden und die anderen Pods über die Änderung benachrichtigt werden
//! (neuer PublicKey, neuer Eintrag in der DB, etc.).

use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    Client,
};

pub async fn is_running_in_k8s() -> bool {
    Client::try_default().await.is_ok()
}

pub async fn k8s_list_pods() -> Result<String, String> {
    let client = Client::try_default().await.map_err(|e| format!("{e:#?}"))?;
    let pods: Api<Pod> = Api::default_namespaced(client);
    let lp = ListParams::default();
    let list = pods.list(&lp).await;
    Ok(format!("{list:#?}"))
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct K8sPeer {
    pub name: String,
    pub ip: String,
    pub namespace: String,
}

// https://stackoverflow.com/questions/57913132
pub async fn k8s_get_peer_ips() -> Result<Vec<K8sPeer>, kube::Error> {
    let client = Client::try_default().await?;
    let pods: Api<Pod> = Api::default_namespaced(client);
    let lp = ListParams::default();
    let list = pods.list(&lp).await;
    Ok(pods
        .list(&lp)
        .await?
        .iter()
        .filter_map(|p| {
            let pod_ip = p.status.as_ref()?.pod_ip.as_ref()?;
            let name = p.metadata.name.as_ref()?;
            Some(K8sPeer {
                name: name.to_string(),
                ip: pod_ip.to_string(),
                namespace: p
                    .metadata
                    .namespace
                    .clone()
                    .unwrap_or("default".to_string()),
            })
        })
        .collect())
}

pub async fn get_sync_server_ip() -> Result<String, String> {
    k8s_get_peer_ips().await
    .map_err(|e| format!("{e}"))?
    .iter()
    .find(|i| i.name.starts_with("dgb-sync"))
    .map(|i| i.ip.clone())
    .ok_or(format!("no pod with name \"dgb-sync\" found"))
}