use crate::api::commit::DbChangeOp;
use crate::db::GpgPublicKeyPair;
use crate::{
    AboLoeschenArgs, AboNeuArgs, BenutzerLoeschenArgs, BenutzerNeuArgsCli, BezirkLoeschenArgs,
    BezirkNeuArgs, SchluesselNeuArgs,
};


pub async fn pull_db_cli() -> Result<(), String> {

    use crate::api::pull::PullResponse;

    let k8s = crate::k8s::is_running_in_k8s().await;
    if !k8s {
        println!("Kubernetes nicht aktiv, pull beendet.");
        return Ok(());
    }

    let k8s_peers = crate::k8s::k8s_get_peer_ips().await
    .map_err(|e| format!("Konnte k8s-peers nicht auslesen: {e}"))?;
    
    let client = reqwest::Client::new();

    for peer in k8s_peers.iter() {
        if !peer.name.starts_with("dgb-server") {
            println!("überspringe: {} (ip {})", peer.name, peer.ip);
            continue;
        }

        let res = client
        .post(&format!("http://{}:8081/pull-db", peer.ip))
        .send()
        .await;

        let (json, bytes) = match res {
            Ok(o) => {
                let bytes = match o.bytes().await {
                    Ok(o) => (&*o).to_vec(),
                    Err(e) => {
                        println!("Pod {} (IP: {}): keine Bytes von /pull-db: {e}", peer.name, peer.ip);
                        continue;
                    }
                };
                let json = serde_json::from_slice::<PullResponse>(&*bytes);
                (json, bytes.to_vec())
            },
            Err(e) => {
                println!("Pod {} (IP: {}): konnte JSON-Antwort von /pull-db nicht lesen: {e}", peer.name, peer.ip);
                continue;
            }
        };

        match json {
            Ok(PullResponse::StatusOk(_)) => {
                println!("Pod {} (IP {}): ok, Datenbank synchronisiert", peer.name, peer.ip);
            }
            Ok(PullResponse::StatusError(e)) => {
                println!("Pod {} (IP {}): Fehler: {}", peer.name, peer.ip, e.text);
            },
            Err(e) => {
                let bytes = String::from_utf8_lossy(&bytes);
                println!("Pod {} (IP {}): Interner Fehler: {e}: {bytes}", peer.name, peer.ip);
            }
        }
    }

    println!("Ok! Datenbank wurde synchronisiert!");
    
    Ok(())
}

pub async fn pull() -> Result<(), String> {

    use git2::Repository;
    use std::path::Path;
    use crate::{get_data_dir, MountPoint};

    let k8s = crate::k8s::is_running_in_k8s().await;
    if !k8s {
        println!("Kubernetes nicht aktiv, pull beendet.");
        return Ok(());
    }

    let local_path = Path::new(&get_data_dir(MountPoint::Local)).to_path_buf();
    if !local_path.exists() {
        let _ = std::fs::create_dir(local_path.clone());
    }

    let repo = match Repository::open(&local_path) {
        Ok(o) => o,
        Err(_) => {
            Repository::init(&local_path)
            .map_err(|e| format!("{e}"))?
        }
    };

    let sync_server_ip = crate::k8s::get_sync_server_ip().await
    .map_err(|e| format!("Konnte Sync-Server nicht finden: {e}"))?;
    
    let data_remote = format!("git://{sync_server_ip}:9418/");
    println!("git clone {data_remote}");
    repo.remote_add_fetch("origin", &data_remote)
        .map_err(|e| format!("git_clone({data_remote}): {e}"))?;
    
    let mut remote = repo.find_remote("origin")
        .map_err(|e| format!("git_clone({data_remote}): {e}"))?;

    remote
        .fetch(&["main"], None, None)
        .map_err(|e| format!("git_clone({data_remote}): {e}"))?;

    let last_commit = repo
        .head()
        .ok()
        .and_then(|c| c.target())
        .and_then(|head_target| repo.find_commit(head_target).ok());

    let commit_id = last_commit.as_ref().map(|l| format!("{}", l.id())).unwrap_or("<leer>".to_string());
    let last_commit_msg = last_commit.as_ref().and_then(|l| l.message()).unwrap_or("");

    println!("Ok, git synchronisiert mit root! Letzter Commit: {commit_id}");
    println!("{last_commit_msg}");
    Ok(())
}

pub async fn create_bezirk_cli(args: &BezirkNeuArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BezirkNeu(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_bezirk_cli(args: &BezirkLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BezirkLoeschen(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub fn schluessel_neu(args: &SchluesselNeuArgs) -> Result<(), anyhow::Error> {
    
    let gpg_key_pair =
        crate::db::create_gpg_key(&args.name, &args.email)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let out_dir = args.dir.clone().unwrap_or(
        std::env::current_dir().ok()
        .and_then(|d| Some(d.canonicalize().ok()?.join("keys")))
        .unwrap_or_default()
    );
    let _ = std::fs::create_dir_all(&out_dir);

    let private_key_out_file = out_dir.join(&format!("{}.private.gpg", args.email));
    std::fs::write(
        private_key_out_file.clone(),
        gpg_key_pair.private.clone().join("\r\n"),
    )?;
    println!("Privater Schlüssel => {private_key_out_file:?}");

    let public_out_file = serde_json::to_string_pretty(&GpgPublicKeyPair {
        fingerprint: gpg_key_pair.fingerprint.clone(),
        public: gpg_key_pair.public.clone(),
    })
    .unwrap_or_default();
    
    let public_key_out_file = out_dir.join(&format!("{}.public.gpg.json", args.email));
    std::fs::write(public_key_out_file.clone(), public_out_file)?;
    println!("Öffentlicher Schlüssel => {public_key_out_file:?}");

    Ok(())
}

pub async fn create_user_cli(args: &BenutzerNeuArgsCli) -> Result<(), anyhow::Error> {
    let benutzer_args_json = args.into_json()?;
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BenutzerNeu(benutzer_args_json), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_user_cli(args: &BenutzerLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::BenutzerLoeschen(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn create_abo_cli(args: &AboNeuArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::AboNeu(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub async fn delete_abo_cli(args: &AboLoeschenArgs) -> Result<(), anyhow::Error> {
    let app_state = crate::load_app_state().await;
    crate::api::write_to_root_db(DbChangeOp::AboLoeschen(args.clone()), &app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
