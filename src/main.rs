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

//
// sudo apt install -y libgpgme-dev libgpg-error-dev
//

use actix_web::{web::JsonConfig, App, HttpServer};
use crate::email::SmtpConfig;
use clap::Parser;

pub mod api;
pub mod db;
pub mod models;
pub mod pgp;
pub mod email;
pub mod index;
pub mod pdf;
pub mod suche;
pub mod k8s;

/// Server für .gbx-Dateien, läuft auf 127.0.0.1:8080
#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Befehl zum Bearbeiten der Datenbark (kein Befehl = Server startet)
    #[clap(subcommand)]
    action: ArgAction,
}

#[derive(clap::Subcommand, Debug, PartialEq)]
pub enum ArgAction {
    /// Starte den Server (--ip, --smtp_host, --smtp_email, --smtp_passwort)
    Start {
        /// IP-Adresse, die der Server verwenden soll
        #[clap(long, default_value = "127.0.0.1")]
        ip: String,
        /// E-Mail-Host um Grundbuchblätter rauszusenden
        #[clap(long, default_value = "")]
        smtp_host: String,
        /// E-Mail Konto, von dem Grundbuchblatt-Änderungen gesendet werden (SMTP)
        #[clap(long, default_value = "")]
        smtp_email: String,
        /// Passwort für SMTP E-Mail Konto für Grundbuchänderungen
        #[clap(long, default_value = "")]
        smtp_passwort: String,   
    },
    /// Starte die Indexierung der Grundbuchblätter als neuen Prozess
    Indexiere,
    /// Suche nach Suchbegriff in momentan vorhandenem Index
    Suche { begriff: String },
    /// Neuen Benutzer anlegen (--name, --email, --passwort, --rechte)
    BenutzerNeu(BenutzerNeuArgs),
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

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
pub struct BenutzerNeuArgs {
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
}

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
pub struct BenutzerLoeschenArgs {
    /// E-Mail des Benutzers, der gelöscht werden soll
    #[clap(short, long)]
    pub email: String,
}

#[derive(clap::Parser, Debug, PartialEq)]
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

#[derive(clap::Parser, Debug, PartialEq)]
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

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
pub struct AboNeuArgs {
    /// Typ des Abonnements ("email" oder "webhook")
    #[clap(short, long)]
    pub typ: String,
    
    /// Name des Amtsgerichts / Gemarkung / Blatts des neuen Abos, 
    /// getrennt mit Schrägstrich ("Prenzlau / Ludwigsburg / 254")
    #[clap(short, long)]
    pub blatt: String,

    /// Name der E-Mail, für die das Abo eingetragen werden soll
    #[clap(short, long)]
    pub email: String,

    /// Aktenzeichen für das neue Abo
    #[clap(short, long)]
    pub aktenzeichen: Option<String>,
}

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
pub struct AboLoeschenArgs {
    /// Typ des Abonnements ("email" oder "webhook")
    #[clap(short, long)]
    pub typ: String,
    
    /// Name des Amtsgerichts / Gemarkung / Blatts des Abos, 
    /// getrennt mit Schrägstrich ("Prenzlau / Ludwigsburg / 254 ")
    #[clap(short, long)]
    pub blatt: String,

    /// Name der E-Mail, für die das Abo eingetragen ist
    #[clap(short, long)]
    pub email: String,

    /// Aktenzeichen des Abonnements
    #[clap(short, long)]
    pub aktenzeichen: Option<String>,
}

pub async fn process_action(action: &ArgAction) -> Result<(), String> {
    use self::ArgAction::*;
    match action {
        Start {
            ip,
            smtp_host,
            smtp_email,
            smtp_passwort,
        } => {
            
            let config = SmtpConfig {
                smtp_adresse: smtp_host.clone(),
                email: smtp_email.clone(),
                passwort: smtp_passwort.clone(),
            };
            
            let _ = init_logger()?;

            let start_sync_mode = std::env::var("SYNC_MODE") == Ok("1".to_string());
            if !start_sync_mode {
                let _ = init(config)?;
                startup_http_server(&ip).await
                .map_err(|e| format!("{e}"))
            } else {
                startup_sync_server(&ip).await
                .map_err(|e| format!("{e}"))
            }
        },
        Indexiere => crate::index::index_all(),
        Suche { begriff } => {
            let suchergebnisse = crate::suche::suche_in_index(&begriff)?;
            println!("{:#?}", suchergebnisse);
            Ok(())
        },
        BenutzerNeu(BenutzerNeuArgs {
            name,
            email,
            passwort,
            rechte,
        }) => crate::db::create_user(name, email, passwort, rechte),
        BenutzerLoeschen(BenutzerLoeschenArgs { email }) => crate::db::delete_user(email),
        BezirkNeu(BezirkNeuArgs {
            land,
            amtsgericht,
            bezirk,
        }) => crate::db::create_gemarkung(land, amtsgericht, bezirk),
        BezirkLoeschen(BezirkLoeschenArgs {
            land,
            amtsgericht,
            bezirk,
        }) => crate::db::delete_gemarkung(land, amtsgericht, bezirk),
        AboNeu(AboNeuArgs {
            typ,
            blatt,
            email,
            aktenzeichen,
        }) => crate::db::create_abo(typ, blatt, email, aktenzeichen.as_ref().map(|s| s.as_str())),
        AboLoeschen(AboLoeschenArgs {
            typ,
            blatt,
            email,
            aktenzeichen,
        }) => crate::db::delete_abo(typ, blatt, email, aktenzeichen.as_ref().map(|s| s.as_str())),
    }
}

fn init(config: SmtpConfig) -> Result<(), String> {
    
    use crate::models::{get_index_dir, get_data_dir, get_keys_dir, get_logs_dir};

    crate::db::create_database()
    .map_err(|e| format!("Fehler in create_database:\r\n{e}"))?;
    
    let _ = std::fs::create_dir_all(get_data_dir());
    let _ = std::fs::create_dir_all(get_index_dir());
    let _ = std::fs::create_dir_all(get_keys_dir());
    let _ = std::fs::create_dir_all(get_logs_dir());

    let _ = crate::email::init_email_config(config);
    
    Ok(())
}

async fn startup_sync_server(ip: &str) -> std::io::Result<()> {    
    HttpServer::new(|| {

        let json_cfg = JsonConfig::default()
            .limit(usize::MAX)
            .content_type_required(false);

        App::new()
            .app_data(json_cfg)
            .wrap(actix_web::middleware::Compress::default())
            .service(crate::api::sync::sync)
    })
    .bind((ip, 8081))?
    .run()
    .await
}

// Server-Start, extra Funktion für Unit-Tests
async fn startup_http_server(ip: &str) -> std::io::Result<()> {    
    HttpServer::new(|| {
    
        let json_cfg = JsonConfig::default()
            .limit(usize::MAX)
            .content_type_required(false);

        App::new()
            .app_data(json_cfg)
            .wrap(actix_web::middleware::Compress::default())
            .service(crate::api::status::status)
            .service(crate::api::status::api)
            .service(crate::api::login::login_get)
            .service(crate::api::login::login_post)
            .service(crate::api::konto::konto_get)
            .service(crate::api::konto::konto_post)
            .service(crate::api::k8s::k8s)
            .service(crate::api::suche::suche)
            .service(crate::api::download::download_gbx)
            .service(crate::api::download::dowload_pdf)
            .service(crate::api::upload::upload)
            .service(crate::api::abo::abo_neu)
            .service(crate::api::abo::abo_loeschen)
        })
    .bind((ip, 8080))?
    .run()
    .await
}

fn init_logger() -> Result<(), String> {

    use slog::*;
    use std::fs::OpenOptions;
    use std::path::Path;
    use crate::models::get_logs_dir;
    
    let _ = std::fs::create_dir_all(get_logs_dir());

    let log_path = Path::new(&get_logs_dir()).join("log.json");
    
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&log_path)
        .map_err(|e| format!("{}: {e}", log_path.display()))?; 
        
    let drain = slog_json::Json::new(file)
        .set_pretty(true)
        .add_default_keys()
        .build()
        .fuse();
        
    let drain = slog_async::Async::new(drain).build().fuse();
    let _ = slog::Logger::root(
        drain, 
        o!(
            "format" => "pretty", 
            "version" => env!("CARGO_PKG_VERSION")
        )
    );
    
    Ok(())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let action = Args::parse().action;
    match process_action(&action).await {
        Ok(_) => Ok(()),
        Err(e) => {
            println!("Fehler: {action:?}:\r\n{e}");
            Ok(())
        }
    }
}
