//! Server für Grundbuch-Daten
//!
//! # API
//!
//! Für alle Operationen, die nur Abfragen betreffen, ist nur eine einfache Authentifizierung
//! notwendig (mit `?email={email}&passwort={passwort}`).
//!
//! - `/suche/{suchbegriff}?email={email}&passwort={passwort}`:
//!   - Sucht nach dem Suchbegriff in den Grundbüchern
//!
//! - `/download/gbx/{land}/{amtsgericht}/{grundbuchbezirk}/{blattnr}?email={email}&passwort={passwort}`
//!   - Gibt den momentanten Stand des jetzigen Grundbuchs als .gbx-Datei per JSON-API aus
//!
//! - `/download/pdf/{land}/{amtsgericht}/{grundbuchbezirk}/{blattnr}?email={email}&passwort={passwort}`
//!   - Gibt den momentanen Stand des jetzigen Grundbuchs als .pdf-Datei aus
//!
//! - `/abo-neu/email/{amtsgericht}/{grundbuchbezirk}/{blattnur}/{tag}?email={email}&passwort={passwort}`
//!   - Erstellt ein neues E-Mail Abonnement für den Benutzer
//!
//! - `/abo-neu/webhook/{amtsgericht}/{grundbuchbezirk}/{blattnur}/{tag}?email={email}&passwort={passwort}`
//!   - Erstellt ein neues Webhook-Abonnement für den Benutzer
//!`
//! - POST `/upload?email={email}&passwort={passwort}`
//!
//! # CLI
//!
//! Der Server speichert alle Benutzerdaten in einer kleinen SQLite-Datenbank, welche
//! nur die Zugriffsdaten und verfügbaren Amtsgerichte enthält.
//!
//! Verfügbare Befehle sind z.B.:
//!
//! - `benutzer-neu`: Legt einen neuen Benutzer an, wichtig sind hierbei die `--rechte`: `"gast"` kann nur lesen,
//!   braucht aber keinen privaten Schlüssel dafür (nur E-Mail und Passwort). `"bearbeiter"` dagegen können, sowohl
//!   lesen als auch Daten bearbeiten und bekommen hierfür eine Schlüsseldatei.
//!
//! - `benutzer-loeschen`: Löscht einen Benutzer (Zugangsdaten werden vernichtet).
//!
//! - `bezirk-neu`: Legt einen neuen Bezirk an. Beim Hochladen der Daten wird die .gbx-Datei auf einen gültigen
//!   Grundbuchbezirk geprüft und dementsprechend im Dateisystem abgelegt
//!
//! - `bezirk-loeschen`: Löscht den angegebenen Grundbuchbezirk.
//!
//! - `abo-neu`: Erstellt ein neues Abonnement mit dem gegebenen Aktenzeichen für ein angegebenes Grundbuchblatt.
//!   Bei Änderungen des Grundbuchblatts wird die angegebene E-Mail im Abonnement benachrichtigt.
//!
//! - `abo-loeschen`: Löscht das angegebene Abonnement.
//!
use crate::{db::GpgPublicKeyPair, email::SmtpConfig, models::MountPoint};
use actix_web::{web::JsonConfig, App, HttpServer};
use clap::Parser;
use models::get_data_dir;
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod api;
pub mod cli;
pub mod db;
pub mod email;
pub mod index;
pub mod k8s;
pub mod models;
pub mod pdf;
pub mod pgp;
pub mod suche;

#[derive(Debug, Clone)]
pub struct AppState {
    pub data: Arc<Mutex<AppStateData>>,
}

impl AppState {
    pub fn host_name(&self) -> String {
        self.data
            .lock()
            .ok()
            .map(|l| l.host_name.clone())
            .unwrap_or_default()
    }
    pub fn sync_server(&self) -> bool {
        self.data
            .lock()
            .ok()
            .map(|l| l.sync_server)
            .unwrap_or(false)
    }
    pub fn k8s_aktiv(&self) -> bool {
        self.data.lock().ok().map(|l| l.k8s_aktiv).unwrap_or(false)
    }
    pub fn smtp_config(&self) -> SmtpConfig {
        self.data
            .lock()
            .ok()
            .map(|l| l.smtp_config.clone())
            .unwrap_or_default()
    }
}

/// Server-interne Konfiguration, geladen beim Server-Start
#[derive(Debug, Clone, PartialEq)]
pub struct AppStateData {
    /// Name des Servers ohne "https://", notwendig für E-Mails,
    /// z.B. "grundbuch-test.eu"
    pub host_name: String,
    /// Ob dieser Server im Sync-Modus läuft (und daher
    /// Schreibrechte auf /mnt/data/files hat) oder nur Lesezugriff
    pub sync_server: bool,
    /// Ob der Server im Kubernetes-Cluster läuft
    pub k8s_aktiv: bool,
    /// Mount des k8s-PersistenVolume zum Synchronisieren zwischen Servern
    pub remote_mount: String,
    /// Konfiguration zum automatischen Senden von Benachrichtigungs-E-Mails
    pub smtp_config: SmtpConfig,
}

/// Server für .gbx-Dateien, läuft auf 127.0.0.1:8080
#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Befehl zum Bearbeiten der Datenbark (kein Befehl = Server startet)
    #[clap(subcommand)]
    action: ArgAction,
}

#[derive(clap::Subcommand, Clone, Debug, PartialEq)]
pub enum ArgAction {
    /// Starte den Server (--ip, --smtp_host, --smtp_email, --smtp_passwort)
    Start {
        /// IP-Adresse, die der Server verwenden soll
        #[clap(long, default_value = "127.0.0.1")]
        ip: String,
    },
    /// Starte die Indexierung der Grundbuchblätter als neuen Prozess
    Indexiere,
    /// Datenbank von Sync-Server lesen
    SyncDb,
    /// Git-Repository von Sync-Server lesen
    Sync,
    /// Suche nach Suchbegriff in momentan vorhandenem Index
    Suche { begriff: String },
    /// Neuen GPG-Schluessel generieren (--name, --email, --dir)
    SchluesselNeu(SchluesselNeuArgs),
    /// Neuen Benutzer anlegen (--name, --email, --passwort, --rechte)
    BenutzerNeu(BenutzerNeuArgsCli),
    /// Benutzer löschen (--email)
    BenutzerLoeschen(BenutzerLoeschenArgs),

    /// Neuen Grundbuchbezirk anlegen (--land, --amtsgericht, --bezirk)
    BezirkNeu(BezirkNeuArgs),
    /// Grundbuchbezirk löschen (--land, --amtsgericht, --bezirk)
    BezirkLoeschen(BezirkLoeschenArgs),

    /// Neues Abonnement anlegen (--typ, --email, --aktenzeichen)
    AboNeu(AboNeuArgs),
    /// Abonnement löschen (--email, --aktenzeichen)
    AboLoeschen(AboLoeschenArgs),
}

#[derive(clap::Parser, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct SchluesselNeuArgs {
    /// Name des neuen Benutzers
    #[clap(short, long)]
    pub name: String,
    /// E-Mail des neuen Benutzers
    #[clap(short, long)]
    pub email: String,
    /// Ausgabeverzeichnis (default: /keys/[email.public.gpg.json] + /keys/[email.private.gpg])
    #[clap(short, long)]
    pub dir: Option<PathBuf>,
}

#[derive(clap::Parser, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct BenutzerNeuArgsCli {
    /// Name des neuen Benutzers
    #[clap(short, long)]
    pub name: String,

    /// E-Mail des neuen Benutzers
    #[clap(short, long)]
    pub email: String,

    /// Passwort des neuen Benutzers
    #[clap(short, long)]
    pub passwort: String,

    /// Rechte (Typ) des neuen Benutzers
    #[clap(short, long, default_value = "gast")]
    pub rechte: String,

    /// Öffentlicher Schlüssel (public key)
    #[clap(short, long)]
    pub schluessel: Option<PathBuf>,
}

impl BenutzerNeuArgsCli {
    pub fn into_json(&self) -> Result<BenutzerNeuArgsJson, anyhow::Error> {
        let schluessel = match self.schluessel.as_ref() {
            Some(s) => {
                let file_contents = std::fs::read_to_string(s)?;
                let parsed = serde_json::from_str(&file_contents)?;
                Some(parsed)
            }
            None => None,
        };

        Ok(BenutzerNeuArgsJson {
            name: self.name.clone(),
            email: self.email.clone(),
            passwort: self.passwort.clone(),
            rechte: self.rechte.clone(),
            schluessel: schluessel,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenutzerNeuArgsJson {
    /// Name des neuen Benutzers
    pub name: String,
    /// E-Mail des neuen Benutzers
    pub email: String,
    /// Passwort des neuen Benutzers
    pub passwort: String,
    /// Rechte (Typ) des neuen Benutzers
    pub rechte: String,
    /// Öffentlicher Schlüssel (public key)
    pub schluessel: Option<GpgPublicKeyPair>,
}

// Aktion um Benutzerdaten zu ändern: None = nicht ändern
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenutzerAendernArgs {
    /// Neuer Name des neuen Benutzers
    pub name: Option<String>,
    /// Neue E-Mail des neuen Benutzers
    pub email: Option<String>,
    /// Neues Passwort des neuen Benutzers
    pub passwort: Option<String>,
    /// Neue Rechte (Typ) des neuen Benutzers
    pub rechte: Option<String>,
    /// Neuer öffentlicher Schlüssel (public key)
    pub schluessel: Option<String>,
}

#[derive(clap::Parser, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct BenutzerLoeschenArgs {
    /// E-Mail des Benutzers, der gelöscht werden soll
    #[clap(short, long)]
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BezirkeNeuArgs {
    pub bezirke: Vec<BezirkNeuArgs>,
}

#[derive(clap::Parser, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct BezirkNeuArgs {
    /// Name des Lands für den neuen Grundbuchbezirk
    #[clap(short, long)]
    pub land: String,

    /// Name des Amtsgerichts für den neuen Grundbuchbezirk
    #[clap(short, long)]
    pub amtsgericht: String,

    /// Name des neuen Grundbuchbezirks
    #[clap(short, long)]
    pub bezirk: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BezirkeLoeschenArgs {
    pub ids: Vec<String>,
}

#[derive(clap::Parser, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct BezirkLoeschenArgs {
    /// Name des Lands des Grundbuchbezirks, der gelöscht werden soll
    #[clap(short, long)]
    pub land: String,

    /// Name des Amtsgerichts des Grundbuchbezirks, der gelöscht werden soll
    #[clap(short, long)]
    pub amtsgericht: String,

    /// Name des Grundbuchbezirks, der gelöscht werden soll
    #[clap(short, long)]
    pub bezirk: String,
}

#[derive(clap::Parser, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct AboNeuArgs {
    /// Typ des Abonnements ("email" oder "webhook")
    #[clap(short, long)]
    pub typ: String,

    /// Name des Amtsgerichts / Gemarkung / Blatts des neuen Abos,
    /// getrennt mit Schrägstrich ("Prenzlau / Ludwigsburg / 254")
    #[clap(short, long)]
    pub blatt: String,

    /// Name der E-Mail / des Webhooks, für die das Abo eingetragen werden soll
    #[clap(short, long)]
    pub text: String,

    /// Aktenzeichen für das neue Abo
    #[clap(short, long)]
    pub aktenzeichen: Option<String>,
}

#[derive(clap::Parser, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[clap(author, version, about, long_about = None)]
pub struct AboLoeschenArgs {
    /// Typ des Abonnements ("email" oder "webhook")
    #[clap(short, long)]
    pub typ: String,

    /// Name des Amtsgerichts / Gemarkung / Blatts des Abos,
    /// getrennt mit Schrägstrich ("Prenzlau / Ludwigsburg / 254 ")
    #[clap(short, long)]
    pub blatt: String,

    /// Name der E-Mail / Webhooks, für die das Abo eingetragen ist
    #[clap(short, long)]
    pub text: String,

    /// Aktenzeichen des Abonnements
    #[clap(short, long)]
    pub aktenzeichen: Option<String>,
}

pub fn process_action(action: &ArgAction) -> Result<(), String> {
    use self::ArgAction::*;
    match action {
        Start { ip } => {
            let _ = init_logger()?;

            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                let app_state = load_app_state().await;
                let _ = init(&app_state).await?;
                if !app_state.sync_server() {
                    startup_http_server(&ip, app_state)
                        .await
                        .map_err(|e| format!("{e}"))
                } else {
                    startup_sync_server(&ip, app_state)
                        .await
                        .map_err(|e| format!("{e}"))
                }
            })
        }
        Indexiere => crate::index::index_all(),
        SyncDb => {
            let _ = init_logger()?;

            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                crate::cli::pull_db_cli().await?;
                Ok(())
            })
        }
        Sync => {
            let _ = init_logger()?;

            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                crate::cli::pull().await?;
                Ok(())
            })
        }
        Suche { begriff } => {
            let suchergebnisse = crate::suche::suche_in_index(&begriff)?;
            println!("{:#?}", suchergebnisse);
            Ok(())
        }
        SchluesselNeu(a) => crate::cli::schluessel_neu(a).map_err(|e| format!("{e}")),
        BenutzerNeu(a) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                crate::cli::create_user_cli(a)
                    .await
                    .map_err(|e| format!("{e}"))
            })
        }
        BenutzerLoeschen(a) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                crate::cli::delete_user_cli(a)
                    .await
                    .map_err(|e| format!("{e}"))
            })
        }
        BezirkNeu(a) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                crate::cli::create_bezirk_cli(a)
                    .await
                    .map_err(|e| format!("{e}"))
            })
        }
        BezirkLoeschen(a) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                crate::cli::delete_bezirk_cli(a)
                    .await
                    .map_err(|e| format!("{e}"))
            })
        }
        AboNeu(a) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                crate::cli::create_abo_cli(a)
                    .await
                    .map_err(|e| format!("{e}"))
            })
        }
        AboLoeschen(a) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("tokio: {e}"))?;

            runtime.block_on(async move {
                crate::cli::delete_abo_cli(a)
                    .await
                    .map_err(|e| format!("{e}"))
            })
        }
    }
}

async fn init(app_state: &AppState) -> Result<(), String> {
    use crate::models::{get_db_path, get_index_dir};
    use git2::Repository;

    if app_state.k8s_aktiv() && !app_state.sync_server() {
        // Warte bis sync-server online ist
        let mut timeout = 0;
        while timeout < 120 {
            timeout += 1;
            if crate::k8s::get_sync_server_ip().await.is_ok() {
                break;
            }
            println!("Warte auf dgb-sync server... ({timeout} / 120 seconds)");
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        let sync_server_ip = crate::k8s::get_sync_server_ip().await?;
        println!("ok dgb-sync = {sync_server_ip}");

        let database_bytes = crate::db::get_db_bytes().await?;
        let _ = std::fs::create_dir_all(get_data_dir(MountPoint::Local));
        let _ = std::fs::create_dir_all(get_index_dir());

        println!("dgb-server: Datenbank erstellt");
        std::fs::write(get_db_path(MountPoint::Local), database_bytes)
            .map_err(|e| format!("Fehler in copy_database:\r\n{e}"))?;

        let data_local = get_data_dir(MountPoint::Local);
        let data_remote = format!("git://{sync_server_ip}:9418/");

        println!("dgb-server git clone {data_remote:?} {data_local:?}");
        let _ = git2::Repository::clone(&data_remote, &data_local).map_err(|e| {
            format!("Fehler in clone_repository({data_remote:?}, {data_local:?}): {e}")
        })?;
        println!("dgb-server: ok, git clone erfolgreich");
    } else if app_state.k8s_aktiv() && app_state.sync_server() {
        println!(
            "dgb-sync: erstelle Datenbank in {:?}",
            get_db_path(MountPoint::Remote)
        );
        crate::db::create_database(MountPoint::Remote)
            .map_err(|e| format!("Fehler in create_database:\r\n{e}"))?;

        let data_dir = get_data_dir(MountPoint::Remote);
        println!("dgb-sync: erstelle /data dir in {data_dir:?}");
        let _ = std::fs::create_dir_all(&data_dir);
        println!("dgb-sync: initialisiere repo in {data_dir:?}");
        match Repository::open(&data_dir) {
            Ok(o) => o,
            Err(_) => Repository::init(&data_dir).map_err(|e| format!("{e}"))?,
        };
    } else {
        crate::db::create_database(MountPoint::Local)
            .map_err(|e| format!("Fehler in create_database:\r\n{e}"))?;
    }

    Ok(())
}

async fn load_app_state() -> AppState {
    AppState {
        data: Arc::new(Mutex::new(AppStateData {
            host_name: std::env::var("HOST_NAME").unwrap_or("127.0.0.1".to_string()),
            sync_server: std::env::var("SYNC_MODE") == Ok("1".to_string()),
            remote_mount: std::env::var("REMOTE_MOUNT").unwrap_or("/mnt/data/files".to_string()),
            k8s_aktiv: crate::k8s::is_running_in_k8s().await,
            smtp_config: SmtpConfig::default(),
        })),
    }
}

async fn startup_sync_server(ip: &str, app_state: AppState) -> std::io::Result<()> {
    use std::path::Path;

    println!(
        "dgb-sync: starte git daemon --base-path={}",
        get_data_dir(MountPoint::Remote)
    );

    std::thread::spawn(move || {
        let git_daemon_ok_file = Path::new(&get_data_dir(MountPoint::Remote))
            .join(".git")
            .join("git-daemon-export-ok");

        let _ = std::fs::write(&git_daemon_ok_file, &[])
            .expect("could not create git-daemon-export-ok file");

        let _ = std::process::Command::new("git")
            .arg("daemon")
            .arg("--reuseaddr")
            .arg(&format!("--base-path={}", get_data_dir(MountPoint::Remote)))
            .arg(&get_data_dir(MountPoint::Remote))
            .output()
            .expect("could not spawm git daemon");
    });

    let k8s_peers = crate::k8s::k8s_get_peer_ips().await.unwrap_or_default();
    println!("\r\nNAME\tIP");
    println!("-------------------------------");
    for p in k8s_peers {
        println!("{}\t{}", p.name, p.ip);
    }

    println!(
        "\r\ndgb-sync: starte sync server (port 8081, endpoint = /commit, /db, /get-db /ping)"
    );

    HttpServer::new(move || {
        let json_cfg = JsonConfig::default()
            .limit(usize::MAX)
            .content_type_required(false);

        App::new()
            .app_data(json_cfg)
            .app_data(actix_web::web::Data::new(app_state.clone()))
            .service(crate::api::commit::commit)
            .service(crate::api::commit::db)
            .service(crate::api::commit::get_db)
            .service(crate::api::commit::ping)
    })
    .bind((ip, 8081))?
    .run()
    .await
}

// Server-Start, extra Funktion für Unit-Tests
async fn startup_http_server(ip: &str, app_state: AppState) -> std::io::Result<()> {
    let json_cfg = || {
        JsonConfig::default()
            .limit(usize::MAX)
            .content_type_required(false)
    };

    println!("dgb-server: starte http server");
    let app_state_clone = app_state.clone();
    let a = async move {
        let app_state_clone = app_state_clone.clone();

        HttpServer::new(move || {
            let app_state_clone = app_state_clone.clone();

            let cors = actix_cors::Cors::permissive()
                .allow_any_origin()
                .supports_credentials();

            App::new()
                .app_data(json_cfg())
                .app_data(actix_web::web::Data::new(app_state_clone))
                .wrap(actix_web::middleware::Compress::default())
                .wrap(cors)
                .service(crate::api::index::status)
                .service(crate::api::index::zugriff)
                .service(crate::api::index::zugriff_post)
                .service(crate::api::index::konto_js)
                .service(crate::api::index::api)
                .service(crate::api::login::login_get)
                .service(crate::api::login::login_post)
                .service(crate::api::konto::konto_get)
                .service(crate::api::konto::konto_post)
                .service(crate::api::konto::konto_generiere_schluessel)
                .service(crate::api::suche::suche)
                .service(crate::api::download::download_gbx)
                .service(crate::api::download::dowload_pdf)
                .service(crate::api::download::dowload_aenderung_pdf)
                .service(crate::api::upload::upload)
                .service(crate::api::abo::abo_neu)
                .service(crate::api::abo::abo_loeschen)
        })
        .bind((ip, 8080))?
        .run()
        .await
    };

    let app_state_clone = app_state.clone();
    let b = async move {
        let app_state_clone = app_state_clone.clone();

        HttpServer::new(move || {
            let app_state_clone = app_state_clone.clone();

            App::new()
                .app_data(json_cfg())
                .app_data(actix_web::web::Data::new(app_state_clone))
                .wrap(actix_web::middleware::Compress::default())
                .service(crate::api::pull::pull)
                .service(crate::api::pull::pull_db)
        })
        .bind((ip, 8081))?
        .run()
        .await
    };

    match tokio::try_join!(a, b) {
        Ok(((), ())) => Ok(()),
        Err(e) => Err(e),
    }
}

fn init_logger() -> Result<(), String> {
    use slog::{o, Drain};

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let _ = slog::Logger::root(
        drain,
        o!(
            "format" => "pretty",
            "version" => env!("CARGO_PKG_VERSION")
        ),
    );

    Ok(())
}

fn main() -> std::io::Result<()> {
    let action = Args::parse().action;
    match process_action(&action) {
        Ok(_) => Ok(()),
        Err(e) => {
            println!("Fehler: {action:?}:\r\n{e}");
            Ok(())
        }
    }
}
