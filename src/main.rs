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
use clap::Parser;

pub mod api;
pub mod db;
pub mod models;
pub mod pgp;
pub mod email;
// pub mod suche;

/// Kommandozeilenargumente
#[derive(clap::Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Ob der Server HTTPS benutzen soll
    #[clap(short, long)]
    https: bool,

    /// Server-interne IP, auf der der Server erreichbar sein soll
    #[clap(short, long, default_value = "127.0.0.1")]
    ip: String,

    /// Befehl zum Bearbeiten der Datenbark (kein Befehl = Server startet)
    #[clap(subcommand)]
    action: Option<ArgAction>,
}

#[derive(clap::Subcommand, Debug, PartialEq)]
enum ArgAction {
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
struct BenutzerNeuArgs {
    /// Name des neuen Benutzers
    #[clap(short, long)]
    name: String,

    /// E-Mail des neuen Benutzers
    #[clap(short, long)]
    email: String,

    /// Passwort des neuen Benutzers
    #[clap(short, long)]
    passwort: String,

    /// Rechte (Typ) des neuen Benutzers
    #[clap(short, long, default_value = "gast")]
    rechte: String,
}

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
struct BenutzerLoeschenArgs {
    /// E-Mail des Benutzers, der gelöscht werden soll
    #[clap(short, long)]
    email: String,
}

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
struct BezirkNeuArgs {
    /// Name des Lands für den neuen Grundbuchbezirk
    #[clap(short, long)]
    land: String,

    /// Name des Amtsgerichts für den neuen Grundbuchbezirk
    #[clap(short, long)]
    amtsgericht: String,

    /// Name des neuen Grundbuchbezirks
    #[clap(short, long)]
    bezirk: String,
}

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
struct BezirkLoeschenArgs {
    /// Name des Lands des Grundbuchbezirks, der gelöscht werden soll
    #[clap(short, long)]
    land: String,

    /// Name des Amtsgerichts des Grundbuchbezirks, der gelöscht werden soll
    #[clap(short, long)]
    amtsgericht: String,

    /// Name des Grundbuchbezirks, der gelöscht werden soll
    #[clap(short, long)]
    bezirk: String,
}

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
struct AboNeuArgs {
    /// Typ des Abonnements ("email" oder "webhook")
    #[clap(short, long)]
    typ: String,
    
    /// Name des Amtsgerichts / Gemarkung / Blatts des neuen Abos, 
    /// getrennt mit Schrägstrich ("Prenzlau / Ludwigsburg / 254")
    #[clap(short, long)]
    blatt: String,

    /// Name der E-Mail, für die das Abo eingetragen werden soll
    #[clap(short, long)]
    email: String,

    /// Aktenzeichen für das neue Abo
    #[clap(short, long)]
    aktenzeichen: String,
}

#[derive(clap::Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
struct AboLoeschenArgs {
    /// Typ des Abonnements ("email" oder "webhook")
    #[clap(short, long)]
    typ: String,
    
    /// Name des Amtsgerichts / Gemarkung / Blatts des Abos, 
    /// getrennt mit Schrägstrich ("Prenzlau / Ludwigsburg / 254 ")
    #[clap(short, long)]
    blatt: String,

    /// Name der E-Mail, für die das Abo eingetragen ist
    #[clap(short, long)]
    email: String,

    /// Aktenzeichen des Abonnements
    #[clap(short, long)]
    aktenzeichen: String,
}

fn process_action(action: &ArgAction) -> Result<(), String> {
    use self::ArgAction::*;
    match action {
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
        }) => crate::db::create_abo(typ, blatt, email, aktenzeichen),
        AboLoeschen(AboLoeschenArgs {
            typ,
            blatt,
            email,
            aktenzeichen,
        }) => crate::db::delete_abo(typ, blatt, email, aktenzeichen),
    }
}

// Server-Start, extra Funktion für Unit-Tests
pub async fn startup_http_server(ip: &str, https: bool) -> std::io::Result<()> {
    HttpServer::new(|| {
        let json_cfg = JsonConfig::default()
            .limit(usize::MAX)
            .content_type_required(false);

        App::new()
            .app_data(json_cfg)
            .service(crate::api::suche::suche)
            .service(crate::api::download::download_gbx)
            .service(crate::api::download::dowload_pdf)
            .service(crate::api::upload::upload)
    })
    .bind((ip, if https { 5431 } else { 8080 }))?
    .run()
    .await
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if let Err(e) = crate::db::create_database() {
        println!("Fehler in create_database:\r\n{e}");
        return Ok(());
    }

    if let Some(action) = args.action.as_ref() {
        if let Err(e) = process_action(action) {
            println!("Fehler in process_action:\r\n{action:#?}:\r\n{e}");
        }
        return Ok(());
    }

    startup_http_server(&args.ip, args.https).await
}
