//! Operationen über die Benutzer-Datenbank

use crate::{
    api::{
        index::ZugriffTyp,
        pull::{PullResponse, PullResponseError},
    },
    email::SmtpConfig,
    models::{get_data_dir, get_db_path, AbonnementInfo, BenutzerInfo, PdfFile},
    BezirkNeuArgs, MountPoint,
};
use chrono::{DateTime, Utc};
use git2::Repository;
use lz4_flex::decompress_size_prepended;
use rusqlite::{Connection, OpenFlags};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type GemarkungsBezirke = Vec<(String, String, String)>;
const PASSWORD_LEN: usize = 128;

pub async fn get_db_bytes() -> Result<Vec<u8>, String> {
    let ip = crate::k8s::get_sync_server_ip().await?;
    let client = reqwest::Client::new();
    let res = client
        .post(&format!("http://{ip}:8081/get-db"))
        .send()
        .await
        .map_err(|e| format!("Konnte Sync-Server nicht erreichen: {e}"))?;

    if res.status() != reqwest::StatusCode::OK {
        return Err(format!(
            "Konnte Datenbank nicht von Sync-Server erhalten: 404"
        ));
    }

    let bytes = res
        .bytes()
        .await
        .map_err(|e| format!("Konnte Datenbank nicht synchronisieren: {e}"))?;

    let bytes = decompress_size_prepended(&bytes)
        .map_err(|e| format!("Fehler beim Dekomprimieren: {e}"))?;

    Ok(bytes)
}

pub async fn pull_db() -> Result<(), PullResponseError> {
    let k8s = crate::k8s::is_running_in_k8s().await;
    if !k8s {
        return Ok(());
    }

    let k8s_peers = crate::k8s::k8s_get_peer_ips()
        .await
        .map_err(|_| PullResponseError {
            code: 500,
            text:
                "Kubernetes aktiv, konnte aber Pods nicht lesen (keine ClusterRole-Berechtigung?)"
                    .to_string(),
        })?;

    for peer in k8s_peers.iter() {
        if !peer.name.starts_with("dgb-server") {
            continue;
        }
        let client = reqwest::Client::new();
        let res = client
            .post(&format!("http://{}:8081/pull-db", peer.ip))
            .send()
            .await;

        let json = match res {
            Ok(o) => o.json::<PullResponse>().await,
            Err(e) => {
                return Err(PullResponseError {
                    code: 500,
                    text: format!(
                        "Pod {}:{} konnte nicht synchronisiert werden: {e}",
                        peer.namespace, peer.name
                    ),
                });
            }
        };

        match json {
            Ok(PullResponse::StatusOk(_)) => {}
            Ok(PullResponse::StatusError(e)) => return Err(e),
            Err(e) => {
                return Err(PullResponseError {
                    code: 500,
                    text: format!(
                        "Pod {}:{} konnte nicht synchronisiert werden: {e}",
                        peer.namespace, peer.name
                    ),
                });
            }
        }
    }

    Ok(())
}

pub fn create_database(mount_point: MountPoint) -> Result<(), rusqlite::Error> {
    let mut open_flags = OpenFlags::empty();

    open_flags.set(OpenFlags::SQLITE_OPEN_NOFOLLOW, true);
    open_flags.set(OpenFlags::SQLITE_OPEN_PRIVATE_CACHE, true);
    open_flags.set(OpenFlags::SQLITE_OPEN_CREATE, true);
    open_flags.set(OpenFlags::SQLITE_OPEN_READ_WRITE, true);

    if let Some(parent) = std::path::Path::new(&get_db_path(mount_point)).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let conn = Connection::open_with_flags(get_db_path(mount_point), open_flags)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS zugriffe (
                id              STRING PRIMARY KEY UNIQUE NOT NULL,
                name            VARCHAR(255) NOT NULL,
                email           VARCHAR(255) NOT NULL,
                typ             VARCHAR(50) NOT NULL,
                grund           STRING,
                land            STRING NOT NULL,
                amtsgericht     STRING NOT NULL,
                bezirk          STRING NOT NULL,
                blatt           STRING NOT NULL,
                angefragt       STRING NOT NULL,
                gewaehrt_von    STRING,
                abgelehnt_von   STRING,
                am              STRING
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS benutzer (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                email           VARCHAR(255) UNIQUE NOT NULL,
                name            VARCHAR(255) NOT NULL,
                rechte          VARCHAR(255) NOT NULL,
                password_hashed BLOB
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
                benutzer        INTEGER,
                token           VARCHAR(1024) UNIQUE NOT NULL,
                gueltig_bis     VARCHAR(255) NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS bezirke (
            id              VARCHAR(255) NOT NULL,
            land            VARCHAR(255) NOT NULL,
            amtsgericht     VARCHAR(255) NOT NULL,
            bezirk          VARCHAR(255) NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS abonnements (
            typ              VARCHAR(50) NOT NULL,
            text             VARCHAR(1023) NOT NULL,
            amtsgericht      VARCHAR(255) NOT NULL,
            bezirk           VARCHAR(255) NOT NULL,
            blatt            VARCHAR(255) NOT NULL,
            aktenzeichen     VARCHAR(1023)
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS publickeys (
                email           VARCHAR(255) NOT NULL,
                pubkey          TEXT NOT NULL,
                fingerprint     VARCHAR(2048) NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS einstellungen (
            id              VARCHAR(255) NOT NULL,
            benutzer        INTEGER NOT NULL,
            einstellung     VARCHAR(1023) NOT NULL,
            wert            STRING NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS grundbuecher (
            land            VARCHAR(255) NOT NULL,
            amtsgericht     VARCHAR(1024) NOT NULL,
            bezirk          VARCHAR(1024) NOT NULL,
            blatt           VARCHAR(20) NOT NULL
        )",
        [],
    )?;

    seed_benutzer_einstellungen(mount_point)?;

    Ok(())
}

pub fn bearbeite_einstellung(
    mount_point: MountPoint,
    einstellung_id: &str,
    neuer_wert: &str,
) -> Result<(), String> {
    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "UPDATE einstellungen SET wert = ?2 WHERE id = ?1",
        rusqlite::params![einstellung_id, neuer_wert],
    )
    .map_err(|_| format!("Fehler beim Bearbeiten der Einstellungen"))?;

    Ok(())
}

pub fn seed_benutzer_einstellungen(mount_point: MountPoint) -> Result<(), rusqlite::Error> {
    let mut conn = Connection::open(get_db_path(mount_point))?;

    let benutzer = {
        let mut stmt = conn.prepare("SELECT id, rechte FROM benutzer")?;

        let benutzer = stmt
            .query_map([], |row| {
                Ok((row.get::<usize, usize>(0)?, row.get::<usize, String>(1)?))
            })?
            .filter_map(|f| f.ok())
            .collect::<BTreeMap<usize, String>>();

        benutzer
    };

    let mut tx = conn.transaction()?;

    {
        let mut prepared = tx.prepare(
            "
                INSERT INTO einstellungen(id,benutzer,einstellung,wert) 
                SELECT ?1, ?2, ?3, ?4
                WHERE NOT EXISTS(SELECT 1 FROM einstellungen WHERE benutzer = ?2 AND einstellung = ?3);
            "
        )?;

        prepared.execute(rusqlite::params![
            generate_uuid(),
            -1,
            "server.config.url",
            "http://127.0.0.1:8080"
        ])?;
        prepared.execute(rusqlite::params![
            generate_uuid(),
            -1,
            "email.out.smtp.address",
            ""
        ])?;
        prepared.execute(rusqlite::params![
            generate_uuid(),
            -1,
            "email.out.smtp.email",
            ""
        ])?;
        prepared.execute(rusqlite::params![
            generate_uuid(),
            -1,
            "email.out.smtp.passwort",
            ""
        ])?;

        prepared.execute(rusqlite::params![
            generate_uuid(),
            -1,
            "download.release.url.windows",
            "https://github.com/wasmerio/wasmer/releases/download/2.3.0/wasmer-windows.exe"
        ])?;
        prepared.execute(rusqlite::params![
            generate_uuid(),
            -1,
            "download.release.url.linux",
            "https://github.com/wasmerio/wasmer/releases/download/2.3.0/wasmer-windows.exe"
        ])?;
        prepared.execute(rusqlite::params![
            generate_uuid(),
            -1,
            "download.release.url.mac",
            "https://github.com/wasmerio/wasmer/releases/download/2.3.0/wasmer-windows.exe"
        ])?;
    }

    tx.commit()?;

    Ok(())
}

pub fn get_globale_einstellungen(
    mount_point: MountPoint,
) -> Result<BTreeMap<String, (String, String)>, String> {
    get_einstellungen_fuer_benutzer(
        mount_point,
        &BenutzerInfo {
            id: -1,
            rechte: "gast".to_string(),
            name: "".to_string(),
            email: "".to_string(),
        },
    )
}

pub fn get_einstellungen_fuer_benutzer(
    mount_point: MountPoint,
    info: &BenutzerInfo,
) -> Result<BTreeMap<String, (String, String)>, String> {
    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT id, einstellung, wert FROM einstellungen WHERE benutzer = ?1")
        .map_err(|_| format!("Fehler beim Auslesen der Einstellungen"))?;

    let einstellungen = stmt
        .query_map(rusqlite::params![info.id], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, String>(1)?,
                row.get::<usize, String>(2)?,
            ))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    Ok(einstellungen
        .into_iter()
        .filter_map(|o| o.ok())
        .map(|(a, b, c)| (a, (b, c)))
        .collect())
}

pub fn bezirke_einfuegen(mount_point: MountPoint, bezirke: &[BezirkNeuArgs]) -> Result<(), String> {
    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Fehler beim Erstellen der Transaktion: {e}"))?;

    tx.execute("DELETE FROM bezirke", rusqlite::params![])
        .map_err(|e| format!("Fehler beim Einfügen von Bezirken in Datenbank: {e}"))?;

    // https://stackoverflow.com/questions/1609637
    for (i, row) in bezirke.iter().enumerate() {
        let land = row.land.replace("\"", "").replace("\'", "");
        let id = crate::db::generate_token().0;
        let land = match Bundesland::from_code(&land) {
            Some(s) => s,
            None => match Bundesland::from_string(&land) {
                Some(s) => s,
                None => {
                    return Err(format!("Ungültiges Bundesland in Zeile {i}"));
                }
            },
        };

        let amtsgericht = row.amtsgericht.replace("\"", "").replace("\'", "");
        let bezirk = row.bezirk.replace("\"", "").replace("\'", "");

        tx.execute(
            "INSERT INTO bezirke (id, land, amtsgericht, bezirk) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, land.into_code(), amtsgericht, bezirk],
        )
        .map_err(|e| format!("Fehler beim Einfügen von Zeile {i} in Datenbank: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("Fehler beim Einfügen von Bezirken in Datenbank: {e}"))?;

    Ok(())
}

pub fn bezirke_loeschen(mount_point: MountPoint, ids: &[String]) -> Result<(), String> {
    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Fehler beim Löschen von Bezirken in Datenbank: {e}"))?;

    for id in ids.iter() {
        tx.execute(
            "DELETE FROM bezirke WHERE id = ?1",
            rusqlite::params![id.clone()],
        )
        .map_err(|e| format!("Fehler beim Löschen von Bezirken in Datenbank: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("Fehler beim Löschen von Bezirken in Datenbank: {e}"))?;

    Ok(())
}

pub fn create_gemarkung(
    mount_point: MountPoint,
    land: &str,
    amtsgericht: &str,
    bezirk: &str,
) -> Result<(), String> {
    let land = land.replace("\"", "").replace("\'", "");
    let land = match Bundesland::from_code(&land) {
        Some(s) => s,
        None => match Bundesland::from_string(&land) {
            Some(s) => s,
            None => {
                return Err(format!("Ungültiges Bundesland"));
            }
        },
    };

    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let id = crate::db::generate_token().0;

    conn.execute(
        "INSERT INTO bezirke (id, land, amtsgericht, bezirk) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, land.into_code(), amtsgericht, bezirk],
    )
    .map_err(|e| {
        format!(
            "Fehler beim Einfügen von {}/{amtsgericht}/{bezirk} in Datenbank: {e}",
            land.into_str()
        )
    })?;

    Ok(())
}

pub fn get_gemarkungen() -> Result<GemarkungsBezirke, String> {
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT land, amtsgericht, bezirk FROM bezirke")
        .map_err(|_| format!("Fehler beim Auslesen der Bezirke"))?;

    let bezirke = stmt
        .query_map([], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, String>(1)?,
                row.get::<usize, String>(2)?,
            ))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut bz = Vec::new();
    for b in bezirke {
        if let Ok(b) = b {
            bz.push(b);
        }
    }
    Ok(bz)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub enum Bundesland {
    BadenWuerttemberg,
    Bayern,
    Berlin,
    Brandenburg,
    Bremen,
    Hamburg,
    Hessen,
    MecklenburgVorpommern,
    Niedersachsen,
    NordrheinWestfalen,
    RheinlandPfalz,
    Saarland,
    Sachsen,
    SachsenAnhalt,
    SchleswigHolstein,
    Thueringen,
}

impl Bundesland {
    pub fn into_code(&self) -> &'static str {
        use self::Bundesland::*;
        match self {
            BadenWuerttemberg => "BWB",
            Bayern => "BYN",
            Berlin => "BLN",
            Brandenburg => "BRA",
            Bremen => "BRE",
            Hamburg => "HAM",
            Hessen => "HES",
            MecklenburgVorpommern => "MPV",
            Niedersachsen => "NSA",
            NordrheinWestfalen => "NRW",
            RheinlandPfalz => "RLP",
            Saarland => "SRL",
            Sachsen => "SAC",
            SachsenAnhalt => "SAA",
            SchleswigHolstein => "SLH",
            Thueringen => "THU",
        }
    }

    pub fn into_str(&self) -> &'static str {
        use self::Bundesland::*;
        match self {
            BadenWuerttemberg => "Baden-Württemberg",
            Bayern => "Bayern",
            Berlin => "Berlin",
            Brandenburg => "Brandenburg",
            Bremen => "Bremen",
            Hamburg => "Hamburg",
            Hessen => "Hessen",
            MecklenburgVorpommern => "Mecklenburg-Vorpommern",
            Niedersachsen => "Niedersachsen",
            NordrheinWestfalen => "Nordrhein-Westfalen",
            RheinlandPfalz => "Rheinland-Pfalz",
            Saarland => "Saarland",
            Sachsen => "Sachsen",
            SachsenAnhalt => "Sachsen-Anhalt",
            SchleswigHolstein => "Schleswig-Holstein",
            Thueringen => "Thüringen",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        use self::Bundesland::*;
        match s {
            "Baden-Württemberg" => Some(BadenWuerttemberg),
            "Bayern" => Some(Bayern),
            "Berlin" => Some(Berlin),
            "Brandenburg" => Some(Brandenburg),
            "Bremen" => Some(Bremen),
            "Hamburg" => Some(Hamburg),
            "Hessen" => Some(Hessen),
            "Mecklenburg-Vorpommern" => Some(MecklenburgVorpommern),
            "Niedersachsen" => Some(Niedersachsen),
            "Nordrhein-Westfalen" => Some(NordrheinWestfalen),
            "Rheinland-Pfalz" => Some(RheinlandPfalz),
            "Saarland" => Some(Saarland),
            "Sachsen" => Some(Sachsen),
            "Sachsen-Anhalt" => Some(SachsenAnhalt),
            "Schleswig-Holstein" => Some(SchleswigHolstein),
            "Thüringen" => Some(Thueringen),
            _ => None,
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        use self::Bundesland::*;
        match code {
            "BWB" => Some(BadenWuerttemberg),
            "BYN" => Some(Bayern),
            "BLN" => Some(Berlin),
            "BRA" => Some(Brandenburg),
            "BRE" => Some(Bremen),
            "HAM" => Some(Hamburg),
            "HES" => Some(Hessen),
            "MPV" => Some(MecklenburgVorpommern),
            "NSA" => Some(Niedersachsen),
            "NRW" => Some(NordrheinWestfalen),
            "RLP" => Some(RheinlandPfalz),
            "SRL" => Some(Saarland),
            "SAC" => Some(Sachsen),
            "SAA" => Some(SachsenAnhalt),
            "SLH" => Some(SchleswigHolstein),
            "THU" => Some(Thueringen),
            _ => None,
        }
    }
}

pub fn get_amtsgerichte_for_bundesland(bundesland: &str) -> Result<Vec<String>, String> {
    let bundesland_clean = match bundesland {
        "ALLE_BUNDESLAENDER" => {
            return {
                Ok(get_gemarkungen()?
                    .into_iter()
                    .map(|(_, ag, _)| ag.clone())
                    .collect())
            }
        }
        other => Bundesland::from_code(other).ok_or(format!("Ungültige Bundesland-ID"))?,
    };

    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT amtsgericht FROM bezirke where land = ?1")
        .map_err(|_| format!("Fehler beim Auslesen der Bezirke"))?;

    let amtsgerichte = stmt
        .query_map([bundesland_clean.into_code()], |row| {
            Ok((row.get::<usize, String>(0)?,))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut amtsgerichte = amtsgerichte
        .into_iter()
        .filter_map(|b| Some(b.ok()?.0))
        .collect::<Vec<_>>();

    amtsgerichte.sort();
    amtsgerichte.dedup();

    Ok(amtsgerichte)
}

pub fn get_bezirke_for_amtsgericht(amtsgericht: &str) -> Result<Vec<String>, String> {
    let amtsgericht_clean = match amtsgericht {
        "ALLE_AMTSGERICHTE" => {
            return {
                Ok(get_gemarkungen()?
                    .into_iter()
                    .map(|(_, _, bezirk)| bezirk.clone())
                    .collect())
            }
        }
        other => other,
    };

    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT bezirk FROM bezirke where amtsgericht = ?1")
        .map_err(|_| format!("Fehler beim Auslesen der Bezirke"))?;

    let bezirke = stmt
        .query_map([amtsgericht_clean], |row| {
            Ok((row.get::<usize, String>(0)?,))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut bezirke = bezirke
        .into_iter()
        .filter_map(|b| Some(b.ok()?.0))
        .collect::<Vec<_>>();

    bezirke.sort();
    bezirke.dedup();

    Ok(bezirke)
}

pub fn get_blaetter_for_bezirk(
    land: &str,
    amtsgericht: &str,
    bezirk: &str,
) -> Result<Vec<String>, String> {
    use std::path::Path;
    let land = match Bundesland::from_code(land) {
        Some(s) => s,
        None => match Bundesland::from_string(land) {
            Some(s) => s,
            None => {
                return Err(format!("Ungültiges Bundesland"));
            }
        },
    };
    let folder = Path::new(&get_data_dir(MountPoint::Local))
        .join(land.into_str())
        .join(amtsgericht)
        .join(bezirk);

    if !folder.exists() || !folder.is_dir() {
        return Ok(Vec::new());
    }

    let paths = std::fs::read_dir(folder).map_err(|e| format!("{e}"))?;
    let mut blaetter = Vec::new();
    for path in paths {
        let path = path.map_err(|e| format!("{e}"))?.path();
        let file = match std::fs::read_to_string(path) {
            Ok(o) => o,
            Err(_) => continue,
        };
        let parsed: PdfFile = serde_json::from_str(&file)
            .map_err(|e| format!("Konnte Bezirk {bezirk} nicht lesen: {e}"))?;
        blaetter.push(parsed.analysiert.titelblatt.blatt.to_string());
    }

    Ok(blaetter)
}

pub fn delete_gemarkung(
    mount_point: MountPoint,
    land: &str,
    amtsgericht: &str,
    bezirk: &str,
) -> Result<(), String> {
    let land = match Bundesland::from_code(land) {
        Some(s) => s,
        None => match Bundesland::from_string(land) {
            Some(s) => s,
            None => {
                return Err(format!("Ungültiges Bundesland"));
            }
        },
    };

    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "DELETE FROM bezirke WHERE land = ?1 AND amtsgericht = ?2 AND bezirk = ?3",
        rusqlite::params![land.into_code(), amtsgericht, bezirk],
    )
    .map_err(|e| {
        format!(
            "Fehler beim Löschen von {}/{amtsgericht}/{bezirk} in Datenbank: {e}",
            land.into_str()
        )
    })?;

    Ok(())
}

pub fn passwort_aendern(
    mount_point: MountPoint,
    email: &str,
    passwort: &str,
) -> Result<(), String> {
    if passwort.len() > 50 {
        return Err(format!("Passwort zu lang"));
    }

    let password_hashed = hash_password(passwort);

    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "UPDATE benutzer SET password_hashed = ?1 WHERE email = ?2",
        rusqlite::params![password_hashed, email],
    )
    .map_err(|e| format!("Fehler beim Einfügen von Benutzer in Datenbank: {e}"))?;

    Ok(())
}

pub fn create_user(
    mount_point: MountPoint,
    name: &str,
    email: &str,
    passwort: &str,
    rechte: &str,
    pubkey: Option<GpgPublicKeyPair>,
) -> Result<(), String> {
    if passwort.len() > 50 {
        return Err(format!("Passwort zu lang"));
    }

    let password_hashed = hash_password(passwort);

    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "INSERT INTO benutzer (email, name, rechte, password_hashed) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![email, name, rechte, password_hashed],
    )
    .map_err(|e| format!("Fehler beim Einfügen von Benutzer in Datenbank: {e}"))?;

    if let Some(public_key) = pubkey.as_ref() {
        conn.execute(
            "INSERT INTO publickeys (email, fingerprint, pubkey) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                email,
                public_key.fingerprint,
                public_key.public.join("\r\n")
            ],
        )
        .map_err(|e| format!("Fehler beim Einfügen von publickey für {email} in Datenbank: {e}"))?;
    }

    Ok(())
}

pub fn bearbeite_benutzer_pubkey(
    mount_point: MountPoint,
    id: &str,
    neuer_pubkey: &str,
) -> Result<(), String> {
    use sequoia_openpgp::parse::{PacketParser, Parse};
    use sequoia_openpgp::Cert;

    let ppr = PacketParser::from_bytes(neuer_pubkey.as_bytes()).map_err(|e| format!("{e}"))?;

    let cert = Cert::try_from(ppr).map_err(|e| format!("{e}"))?;

    let fingerprint = cert.fingerprint().to_string();

    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let ok_update = conn
        .execute(
            "UPDATE publickeys SET pubkey = ?2, fingerprint = ?3 WHERE email = ?1",
            rusqlite::params![id, neuer_pubkey, fingerprint],
        )
        .ok();

    if ok_update.is_none() || ok_update == Some(0) {
        conn.execute(
            "INSERT INTO publickeys (email, pubkey, fingerprint) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, neuer_pubkey, fingerprint],
        )
        .map_err(|e| format!("Fehler beim Bearbeiten des öffentlichen Schlüssels: {e}"))?;
    }

    Ok(())
}

pub fn bearbeite_benutzer_rechte(
    mount_point: MountPoint,
    ids: &[String],
    neue_rechte: &str,
) -> Result<(), String> {
    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut tx = conn
        .transaction()
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    {
        let mut stmt = tx
            .prepare("UPDATE benutzer SET rechte = ?2 WHERE email = ?1")
            .map_err(|e| format!("Fehler beim Bearbeiten der Benutzerrechte"))?;

        for id in ids {
            let id = id.clone();
            let neue_rechte = neue_rechte.to_string();
            stmt.execute(rusqlite::params![id, neue_rechte])
                .map_err(|e| format!("Fehler beim Bearbeiten der Benutzerrechte"))?;
        }
    }

    tx.commit()
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    Ok(())
}

// given a password, returns the (automatically salted) hash
pub fn hash_password(password: &str) -> Vec<u8> {
    use argon2::{
        password_hash::rand_core::OsRng,
        password_hash::SaltString, 
        PasswordHasher, Argon2,
    };

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    match argon2.hash_password(password.as_bytes(), &salt) {
        Ok(o) => o.serialize().as_bytes().to_vec(),
        Err(_) => vec![0; PASSWORD_LEN],
    }
}

pub fn verify_password(database_pw: &[u8], password: &str) -> bool {
    use argon2::PasswordVerifier;
    use argon2::{password_hash::PasswordHash, Argon2};

    let parsed_hash = match PasswordHash::new(core::str::from_utf8(database_pw).unwrap_or("")) {
        Ok(o) => o,
        Err(_) => return false,
    };
    let argon2 = Argon2::default();
    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Gleicht `GpgKeyPair`, nur ohne den privaten Schlüssel
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GpgPublicKeyPair {
    pub fingerprint: String,
    pub public: Vec<String>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GpgKeyPair {
    pub fingerprint: String,
    pub public: Vec<String>,
    pub private: Vec<String>,
}

pub fn create_gpg_key(name: &str, email: &str) -> Result<GpgKeyPair, String> {
    use sequoia_openpgp::serialize::SerializeInto;

    let key = crate::pgp::generate(&format!("{name} <{email}>"))
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {e}"))?;

    let bytes = key
        .as_tsk()
        .armored()
        .to_vec()
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;

    let secret_key = String::from_utf8(bytes)
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;

    let public_key_bytes = key
        .armored()
        .to_vec()
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;

    let public_key_str = String::from_utf8(public_key_bytes)
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;

    let fingerprint = key.fingerprint().to_string();

    Ok(GpgKeyPair {
        fingerprint,
        public: public_key_str.lines().map(|s| s.to_string()).collect(),
        private: secret_key.lines().map(|s| s.to_string()).collect(),
    })
}

pub fn get_key_for_fingerprint(
    fingerprint: &str,
    email: &str,
) -> Result<sequoia_openpgp::Cert, String> {
    use sequoia_openpgp::parse::PacketParser;
    use sequoia_openpgp::parse::Parse;
    use sequoia_openpgp::Cert;

    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT pubkey FROM publickeys WHERE email = ?1 AND fingerprint = ?2")
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten 10"))?;

    let pubkeys = stmt
        .query_map(rusqlite::params![email, fingerprint], |row| {
            Ok(row.get::<usize, String>(0)?)
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();

    let pubkey = match pubkeys.get(0) {
        Some(Ok(s)) => s,
        _ => return Err(format!("Kein öffentlicher Schlüssel für E-Mail {email:?} / Fingerprint {fingerprint:?} gefunden")),
    };

    let ppr = PacketParser::from_bytes(pubkey.as_bytes()).map_err(|e| format!("{e}"))?;

    let cert = Cert::try_from(ppr).map_err(|e| format!("{e}"))?;

    Ok(cert)
}

pub enum KontoDataResult {
    // Benutzerkonto wurde noch nicht aktiviert (neues Konto)
    KeinPasswort,
    // Benutzerkonto ist normal aktiviert
    Aktiviert(KontoData),
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KontoData {
    pub kontotyp: String,
    pub ausgewaehlt: Option<String>,
    pub data: BTreeMap<String, KontoTabelle>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KontoTabelle {
    pub spalten: Vec<String>,
    pub daten: BTreeMap<String, Vec<String>>,
}

pub fn get_server_address(mount_point: MountPoint) -> Result<String, String> {
    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.query_row(
        "SELECT wert FROM einstellungen WHERE benutzer = -1 AND einstellung = 'server.config.url'",
        rusqlite::params![],
        |r| r.get(0),
    )
    .map_err(|_| format!("Fehler beim Auslesen der Einstellungen"))
}

pub fn get_email_config() -> Result<SmtpConfig, String> {
    let global = get_globale_einstellungen(MountPoint::Local)?;
    let global = global
        .into_iter()
        .map(|(_, (k, v))| (k, v))
        .collect::<BTreeMap<_, _>>();

    let smtp_adresse = global
        .get("email.out.smtp.address")
        .ok_or("E-Mail: keine SMTP-Adresse".to_string())?
        .clone();
    let email = global
        .get("email.out.smtp.email")
        .ok_or("E-Mail: keine SMTP-EMail".to_string())?
        .clone();
    let passwort = global
        .get("email.out.smtp.passwort")
        .ok_or("E-Mail: keine SMTP-EMail".to_string())?
        .clone();

    Ok(SmtpConfig {
        smtp_adresse,
        email,
        passwort,
    })
}

pub fn get_active_publickey_for_benutzer(
    benutzer: &BenutzerInfo,
) -> Result<Option<String>, String> {
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.query_row(
        "SELECT fingerprint FROM publickeys WHERE email = ?1",
        rusqlite::params![benutzer.email],
        |r| r.get::<usize, Option<String>>(0),
    )
    .map_err(|_| format!("Fehler beim Auslesen der Einstellungen"))
}

pub fn get_konto_data(benutzer_info: &BenutzerInfo) -> Result<KontoDataResult, String> {
    let mut data = KontoData::default();
    data.kontotyp = benutzer_info.rechte.clone();

    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let pw_rows: Option<Vec<u8>> = conn
        .query_row(
            "SELECT password_hashed FROM benutzer WHERE id = ?1",
            rusqlite::params![benutzer_info.id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    if pw_rows.is_none() {
        return Ok(KontoDataResult::KeinPasswort);
    }

    match benutzer_info.rechte.as_str() {
        "admin" => {
            // Zugriffe
            let mut stmt = conn
                .prepare(
                    "
                SELECT 
                    id,
                    name,
                    email,
                    typ,
                    grund,
                    land,
                    amtsgericht,
                    bezirk,
                    blatt,
                    angefragt,
                    gewaehrt_von,
                    abgelehnt_von,
                    am
                FROM zugriffe 
            ",
                )
                .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten 1"))?;

            let zugriffe = stmt
                .query_map(rusqlite::params![], |row| {
                    Ok((
                        row.get::<usize, String>(0)?,
                        row.get::<usize, String>(1)?,
                        row.get::<usize, String>(2)?,
                        row.get::<usize, String>(3)?,
                        row.get::<usize, Option<String>>(4)?,
                        row.get::<usize, String>(5)?,
                        row.get::<usize, String>(6)?,
                        row.get::<usize, String>(7)?,
                        row.get::<usize, String>(8)
                            .or_else(|_| row.get::<usize, i32>(8).map(|s| format!("{s}")))?,
                        row.get::<usize, String>(9)?,
                        row.get::<usize, Option<String>>(10)?,
                        row.get::<usize, Option<String>>(11)?,
                        row.get::<usize, Option<String>>(12)?,
                    ))
                })
                .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
                .collect::<Vec<_>>();

            data.data.insert(
                "zugriffe".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "id".to_string(),
                        "name".to_string(),
                        "email".to_string(),
                        "typ".to_string(),
                        "grund".to_string(),
                        "land".to_string(),
                        "amtsgericht".to_string(),
                        "bezirk".to_string(),
                        "blatt".to_string(),
                        "angefragt".to_string(),
                        "gewaehrt_von".to_string(),
                        "abgelehnt_von".to_string(),
                        "am".to_string(),
                    ],
                    daten: zugriffe
                        .into_iter()
                        .filter_map(|row| {
                            let row = row.ok()?;
                            Some((
                                row.0.clone(),
                                vec![
                                    row.0.clone(),
                                    row.1.clone(),
                                    row.2.clone(),
                                    row.3.clone(),
                                    row.4.clone().unwrap_or_default(),
                                    row.5.clone(),
                                    row.6.clone(),
                                    row.7.clone(),
                                    row.8.clone(),
                                    row.9.clone(),
                                    row.10.clone().unwrap_or_default(),
                                    row.11.clone().unwrap_or_default(),
                                    row.12.clone().unwrap_or_default(),
                                ],
                            ))
                        })
                        .collect(),
                },
            );

            // Benutzer
            //
            // https://www.sqlitetutorial.net/sqlite-full-outer-join/
            let mut stmt = conn
                .prepare(
                    "
                SELECT  benutzer.name,
                        benutzer.email,
                        benutzer.rechte,
                        publickeys.pubkey,
                        publickeys.fingerprint
                FROM benutzer
                LEFT JOIN publickeys ON publickeys.email = benutzer.email
                    UNION ALL
                    SELECT benutzer.name,
                        benutzer.email,
                        benutzer.rechte,
                        publickeys.pubkey,
                        publickeys.fingerprint
                    FROM publickeys
                    LEFT JOIN benutzer ON benutzer.email = publickeys.email
                    WHERE publickeys.email IS NULL
            ",
                )
                .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten 2: {e}"))?;

            let benutzer = stmt
                .query_map(rusqlite::params![], |row| {
                    Ok((
                        row.get::<usize, String>(0)?,
                        row.get::<usize, String>(1)?,
                        row.get::<usize, String>(2)?,
                        row.get::<usize, Option<String>>(3)?,
                        row.get::<usize, Option<String>>(4)?,
                    ))
                })
                .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
                .collect::<Vec<_>>();

            data.data.insert(
                "benutzer".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "name".to_string(),
                        "email".to_string(),
                        "rechte".to_string(),
                        "publickeys.fingerprint".to_string(),
                        "publickeys.pubkey".to_string(),
                    ],
                    daten: benutzer
                        .into_iter()
                        .filter_map(|row| {
                            let row = row.ok()?;
                            Some((
                                row.1.clone(),
                                vec![
                                    row.0.clone(),
                                    row.1.clone(),
                                    row.2.clone(),
                                    row.3.clone().unwrap_or_default(),
                                    row.4.clone().unwrap_or_default(),
                                ],
                            ))
                        })
                        .collect(),
                },
            );

            let mut stmt = conn
                .prepare("SELECT id, land, amtsgericht, bezirk FROM bezirke")
                .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten 3"))?;

            let bezirke = stmt
                .query_map(rusqlite::params![], |row| {
                    Ok((
                        row.get::<usize, String>(0)?,
                        row.get::<usize, String>(1)?,
                        row.get::<usize, String>(2)?,
                        row.get::<usize, String>(3)?,
                    ))
                })
                .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
                .collect::<Vec<_>>();

            data.data.insert(
                "bezirke".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "id".to_string(),
                        "land".to_string(),
                        "amtsgericht".to_string(),
                        "bezirk".to_string(),
                    ],
                    daten: bezirke
                        .into_iter()
                        .enumerate()
                        .filter_map(|(i, row)| {
                            let row = row.ok()?;
                            let bundesland = Bundesland::from_code(&row.1)?.into_str().to_string();
                            Some((
                                format!("{i}"),
                                vec![row.0.clone(), bundesland, row.2.clone(), row.3.clone()],
                            ))
                        })
                        .collect(),
                },
            );

            let aenderungen = crate::db::get_aenderungen(AenderungFilter::GetLast(500));
            data.data.insert(
                "aenderungen".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "id".to_string(),
                        "name".to_string(),
                        "email".to_string(),
                        "zeit-sec".to_string(),
                        "zeit-offset".to_string(),
                        "zeit-tz".to_string(),
                        "titel".to_string(),
                        "beschreibung".to_string(),
                    ],
                    daten: aenderungen
                        .into_iter()
                        .enumerate()
                        .map(|(i, a)| (i.to_string(), a))
                        .collect(),
                },
            );

            // Grundbücher
            let verfuegbare_grundbuecher =
                crate::db::get_verfuegbare_grundbuecher_fuer_benutzer(&benutzer_info)
                    .unwrap_or_default();

            data.data.insert(
                "blaetter".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "land".to_string(),
                        "amtsgericht".to_string(),
                        "bezirk".to_string(),
                        "blatt".to_string(),
                    ],
                    daten: verfuegbare_grundbuecher
                        .into_iter()
                        .map(|(l, a, g, b)| (format!("{l}/{a}/{g}/{b}"), vec![l, a, g, b]))
                        .collect(),
                },
            );

            // Abonnements
            let abos =
                crate::db::get_email_abonnements_fuer_benutzer(&benutzer_info).unwrap_or_default();

            data.data.insert(
                "abonnements".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "amtsgericht".to_string(),
                        "bezirk".to_string(),
                        "blatt".to_string(),
                        "aktenzeichen".to_string(),
                    ],
                    daten: abos
                        .into_iter()
                        .map(|(l, a, g, b)| (format!("{l}/{a}/{g}/{b}"), vec![l, a, g, b]))
                        .collect(),
                },
            );

            // Einstellungen
            data.data.insert(
                "meine-kontodaten".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "id".to_string(),
                        "global".to_string(),
                        "einstellung".to_string(),
                        "wert".to_string(),
                    ],
                    daten: {
                        let mut d = BTreeMap::new();
                        for (id, (k, v)) in
                            get_einstellungen_fuer_benutzer(MountPoint::Local, &benutzer_info)?
                                .into_iter()
                        {
                            d.insert(
                                id.to_string(),
                                vec!["false".to_string(), k.to_string(), v.to_string()],
                            );
                        }
                        for (id, (k, v)) in
                            get_globale_einstellungen(MountPoint::Local)?.into_iter()
                        {
                            d.insert(
                                id.to_string(),
                                vec!["true".to_string(), k.to_string(), v.to_string()],
                            );
                        }
                        d
                    },
                },
            );
        }
        "bearbeiter" => {
            // Extra Kontodaten
            let publickey = match crate::db::get_active_publickey_for_benutzer(&benutzer_info) {
                Ok(o) => o,
                Err(_) => None,
            };

            data.data.insert(
                "kontodaten-extra".to_string(),
                KontoTabelle {
                    spalten: vec!["wert".to_string()],
                    daten: {
                        let mut b = BTreeMap::new();
                        b.insert(
                            "konto.publickey".to_string(),
                            vec![publickey.unwrap_or_default()],
                        );
                        b.insert("konto.email".to_string(), vec![benutzer_info.email.clone()]);
                        b.insert("konto.name".to_string(), vec![benutzer_info.name.clone()]);
                        b
                    },
                },
            );

            // Änderungen des Bearbeiters
            let aenderungen = crate::db::get_aenderungen(AenderungFilter::FilterEmail(
                benutzer_info.email.clone(),
            ));
            data.data.insert(
                "aenderungen".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "id".to_string(),
                        "name".to_string(),
                        "email".to_string(),
                        "zeit-sec".to_string(),
                        "zeit-offset".to_string(),
                        "zeit-tz".to_string(),
                        "zusammenfassung".to_string(),
                    ],
                    daten: aenderungen
                        .into_iter()
                        .filter(|a| a[2] == benutzer_info.email)
                        .enumerate()
                        .map(|(i, a)| (i.to_string(), a))
                        .collect(),
                },
            );

            // Grundbücher
            let verfuegbare_grundbuecher =
                crate::db::get_verfuegbare_grundbuecher_fuer_benutzer(&benutzer_info)
                    .unwrap_or_default();

            data.data.insert(
                "blaetter".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "land".to_string(),
                        "amtsgericht".to_string(),
                        "bezirk".to_string(),
                        "blatt".to_string(),
                    ],
                    daten: verfuegbare_grundbuecher
                        .into_iter()
                        .map(|(l, a, g, b)| (format!("{l}/{a}/{g}/{b}"), vec![l, a, g, b]))
                        .collect(),
                },
            );

            // Abonnements
            let abos =
                crate::db::get_email_abonnements_fuer_benutzer(&benutzer_info).unwrap_or_default();

            data.data.insert(
                "abonnements".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "amtsgericht".to_string(),
                        "bezirk".to_string(),
                        "blatt".to_string(),
                        "aktenzeichen".to_string(),
                    ],
                    daten: abos
                        .into_iter()
                        .map(|(l, a, g, b)| (format!("{l}/{a}/{g}/{b}"), vec![l, a, g, b]))
                        .collect(),
                },
            );

            // Einstellungen
            data.data.insert(
                "meine-kontodaten".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "id".to_string(),
                        "global".to_string(),
                        "einstellung".to_string(),
                        "wert".to_string(),
                    ],
                    daten: {
                        let mut d = BTreeMap::new();
                        for (id, (k, v)) in
                            get_einstellungen_fuer_benutzer(MountPoint::Local, &benutzer_info)?
                                .into_iter()
                        {
                            d.insert(
                                id.to_string(),
                                vec!["false".to_string(), k.to_string(), v.to_string()],
                            );
                        }
                        d
                    },
                },
            );
        }
        "gast" => {
            // Grundbücher
            let verfuegbare_grundbuecher =
                crate::db::get_verfuegbare_grundbuecher_fuer_benutzer(&benutzer_info)
                    .unwrap_or_default();

            data.data.insert(
                "blaetter".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "land".to_string(),
                        "amtsgericht".to_string(),
                        "bezirk".to_string(),
                        "blatt".to_string(),
                    ],
                    daten: verfuegbare_grundbuecher
                        .into_iter()
                        .map(|(l, a, g, b)| (format!("{l}/{a}/{g}/{b}"), vec![l, a, g, b]))
                        .collect(),
                },
            );

            // Abonnements
            let abos =
                crate::db::get_email_abonnements_fuer_benutzer(&benutzer_info).unwrap_or_default();

            data.data.insert(
                "abonnements".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "amtsgericht".to_string(),
                        "bezirk".to_string(),
                        "blatt".to_string(),
                        "aktenzeichen".to_string(),
                    ],
                    daten: abos
                        .into_iter()
                        .map(|(l, a, g, b)| (format!("{l}/{a}/{g}/{b}"), vec![l, a, g, b]))
                        .collect(),
                },
            );

            // Einstellungen
            data.data.insert(
                "meine-kontodaten".to_string(),
                KontoTabelle {
                    spalten: vec![
                        "id".to_string(),
                        "global".to_string(),
                        "einstellung".to_string(),
                        "wert".to_string(),
                    ],
                    daten: {
                        let mut d = BTreeMap::new();
                        for (id, (k, v)) in
                            get_einstellungen_fuer_benutzer(MountPoint::Local, &benutzer_info)?
                                .into_iter()
                        {
                            d.insert(
                                id.to_string(),
                                vec!["false".to_string(), k.to_string(), v.to_string()],
                            );
                        }
                        d
                    },
                },
            );
        }
        _ => {}
    }

    Ok(KontoDataResult::Aktiviert(data))
}

#[derive(Debug, Clone, PartialEq)]
pub enum AenderungFilter {
    GetLast(usize),
    FilterEmail(String),
}

pub fn get_aenderungen(filter: AenderungFilter) -> Vec<Vec<String>> {
    let repo = match Repository::open(get_data_dir(MountPoint::Local)) {
        Ok(s) => s,
        Err(e) => return Vec::new(),
    };

    let head = repo
        .head()
        .ok()
        .and_then(|c| c.target())
        .and_then(|head_target| repo.find_commit(head_target).ok());

    let head = match head {
        Some(s) => s,
        None => return Vec::new(),
    };

    let commits = vec![
        format!("{}", head.id()),
        head.author().name().unwrap_or("").to_string(),
        head.author().email().unwrap_or("").to_string(),
        head.author().when().seconds().to_string(),
        head.author().when().offset_minutes().to_string(),
        head.author().when().sign().to_string(),
        head.summary().map(|s| s.to_string()).unwrap_or_default(),
        head.message().map(|s| s.to_string()).unwrap_or_default(),
    ];

    let commits = match filter {
        AenderungFilter::GetLast(i) => {
            let mut v = if i == 0 { Vec::new() } else { vec![commits] };

            if i <= 1 {
                return v;
            }

            v.extend(head.parents().take(i - 1).map(|c| {
                vec![
                    format!("{}", c.id()),
                    c.author().name().unwrap_or("").to_string(),
                    c.author().email().unwrap_or("").to_string(),
                    c.author().when().seconds().to_string(),
                    c.author().when().offset_minutes().to_string(),
                    c.author().when().sign().to_string(),
                    c.summary().map(|s| s.to_string()).unwrap_or_default(),
                ]
            }));

            v
        }
        AenderungFilter::FilterEmail(s) => {
            let mut v = if s == commits[2] {
                vec![commits]
            } else {
                Vec::new()
            };

            v.extend(head.parents().filter_map(|c| {
                let author = c.author();
                let email = author.email().unwrap_or("");
                if email != s {
                    return None;
                } else {
                    Some(vec![
                        format!("{}", c.id()),
                        c.author().name().unwrap_or("").to_string(),
                        email.to_string(),
                        c.author().when().seconds().to_string(),
                        c.author().when().offset_minutes().to_string(),
                        c.author().when().sign().to_string(),
                        c.summary().map(|s| s.to_string()).unwrap_or_default(),
                    ])
                }
            }));

            v
        }
    };

    commits
}

pub fn get_user_from_token(token: &str) -> Result<BenutzerInfo, String> {
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT benutzer, gueltig_bis FROM sessions WHERE token = ?1")
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten 6"))?;

    let tokens = stmt
        .query_map(rusqlite::params![token], |row| {
            Ok((row.get::<usize, i32>(0)?, row.get::<usize, String>(1)?))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();

    let result = tokens.get(0).and_then(|t| {
        t.as_ref()
            .ok()
            .and_then(|(id, g)| Some((id, DateTime::parse_from_rfc3339(&g).ok()?)))
    });

    let id = match result {
        Some((id, gueltig_bis)) => {
            let now = Utc::now();
            if now > gueltig_bis {
                return Err(format!("Token abgelaufen"));
            }
            id
        }
        None => {
            return Err(format!("Ungültiges Token"));
        }
    };

    let mut stmt = conn
        .prepare("SELECT name, email, rechte FROM benutzer WHERE id = ?1")
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten 6"))?;

    let benutzer = stmt
        .query_map(rusqlite::params![id], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, String>(1)?,
                row.get::<usize, String>(2)?,
            ))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();

    let (name, email, rechte) = match benutzer.get(0) {
        Some(Ok(s)) => s,
        _ => return Err(format!("Kein Benutzerkonto für Token vorhanden")),
    };

    Ok(BenutzerInfo {
        id: *id,
        name: name.to_string(),
        email: email.to_string(),
        rechte: rechte.to_string(),
    })
}

pub fn zugriff_ablehnen(
    mount_point: MountPoint,
    ids: &[String],
    email: &str,
    datum: &str,
) -> Result<(), String> {
    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank: {e}"))?;

    for id in ids.iter() {
        tx.execute(
            "UPDATE zugriffe SET gewaehrt_von = NULL, abgelehnt_von = ?1, am = ?2 WHERE id  = ?3",
            rusqlite::params![email.clone(), datum.clone(), id.clone()],
        )
        .map_err(|e| format!("Fehler beim Genehmigen vom Zugriffen: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("Fehler beim Genehmigen vom Zugriffen: {e}"))?;

    Ok(())
}

pub fn zugriff_genehmigen(
    mount_point: MountPoint,
    ids: &[String],
    email: &str,
    datum: &str,
) -> Result<(), String> {
    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank 0: {e}"))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank 1: {e}"))?;

    for id in ids.iter() {
        tx.execute(
            "UPDATE zugriffe SET abgelehnt_von = NULL, gewaehrt_von = ?1, am = ?2 WHERE id = ?3",
            rusqlite::params![email.clone(), datum.clone(), id.clone()],
        )
        .map_err(|e| format!("Fehler beim Genehmigen vom Zugriffen: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank 4: {e}"))?;

    // Benutzer erstellen mit passwort = NULL (wird beim ersten Login gesetzt)
    let mut benutzer_neu = Vec::new();
    for id in ids.iter() {
        let (name, email, typ): (String, String, String) = conn
            .query_row(
                "SELECT name, email, typ FROM zugriffe WHERE id = ?1",
                rusqlite::params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| format!("{e}"))?;

        let query_benutzer_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM benutzer WHERE email = ?1",
                rusqlite::params![email],
                |row| row.get(0),
            )
            .map_err(|e| format!("{e}"))?;

        if query_benutzer_count == 0 {
            benutzer_neu.push((name, email, typ));
        }
    }

    if benutzer_neu.is_empty() {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank 5: {e}"))?;

    if let Ok(mut stmt) = tx.prepare(
        "INSERT INTO benutzer (name, email, rechte, password_hashed) VALUES (?1, ?2, ?3, NULL)",
    ) {
        for (name, email, typ) in benutzer_neu {
            stmt.execute(rusqlite::params![
                name,
                email,
                match typ.as_str() {
                    "bearbeiter" => "bearbeiter",
                    _ => "gast",
                },
            ])
            .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank 6: {e}"))?;
        }
    }

    tx.commit()
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank 7: {e}"))?;

    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct BenutzerGrundbuecher {
    pub name: String,
    pub zugriff_id: String,
    pub rechte: String,
    /// (Bundesland, Amtsgericht, Bezirk, Blatt)
    pub grundbuecher: Vec<(String, String, String, String)>,
}

pub fn get_email_abonnements_fuer_benutzer(
    benutzer: &BenutzerInfo,
) -> Result<Vec<(String, String, String, String)>, String> {
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let is_admin = benutzer.rechte == "admin";
    let mut stmt = conn.prepare(if is_admin {
        "SELECT amtsgericht, bezirk, blatt, aktenzeichen FROM abonnements WHERE typ = 'email'"
    } else {
        "SELECT amtsgericht, bezirk, blatt, aktenzeichen FROM abonnements WHERE typ = 'email' AND text = ?1"
    }).map_err(|e| format!("{e}"))?;

    let pa = rusqlite::params![benutzer.email];
    let pb = rusqlite::params![];
    let params = if is_admin { pa } else { pb };
    let abos = stmt
        .query_map(params, |r| {
            Ok((
                r.get::<usize, String>(0)?,
                r.get::<usize, String>(1)?,
                r.get::<usize, String>(2)?,
                r.get::<usize, Option<String>>(3)?.unwrap_or_default(),
            ))
        })
        .map_err(|e| format!("{e}"))?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{e}"))?;

    Ok(abos)
}

/// (Bundesland, Amtsgericht, Bezirk, Blatt)
pub fn get_verfuegbare_grundbuecher_fuer_benutzer(
    benutzer: &BenutzerInfo,
) -> Result<Vec<(String, String, String, String)>, String> {
    use std::collections::BTreeSet;

    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let is_admin = benutzer.rechte == "admin";
    let mut alle_grundbuecher = conn
        .prepare("SELECT land, amtsgericht, bezirk, blatt FROM grundbuchblaetter")
        .map_err(|e| format!("{e}"))?;

    let grundbuchblaetter = alle_grundbuecher
        .query_map(rusqlite::params![], |r| {
            Ok((
                r.get::<usize, String>(0)?,
                r.get::<usize, String>(1)?,
                r.get::<usize, String>(2)?,
                r.get::<usize, String>(3)?,
            ))
        })
        .map_err(|e| format!("{e}"))?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{e}"))?;

    if grundbuchblaetter.is_empty() {
        return Ok(Vec::new());
    }

    let grundbuchblaetter = grundbuchblaetter.into_iter().collect::<BTreeSet<_>>();
    if is_admin {
        return Ok(grundbuchblaetter.into_iter().collect::<Vec<_>>());
    }

    let mut stmt = conn
        .prepare("SELECT land, amtsgericht, bezirk, blatt FROM zugriffe WHERE email = ?1")
        .map_err(|e| format!("{e}"))?;

    let zugriffe = stmt
        .query_map(rusqlite::params![benutzer.email], |r| {
            Ok((
                r.get::<usize, String>(0)?,
                r.get::<usize, String>(1)?,
                r.get::<usize, String>(2)?,
                r.get::<usize, String>(3)?,
            ))
        })
        .map_err(|e| format!("{e}"))?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{e}"))?;

    if zugriffe.is_empty() {
        return Ok(Vec::new());
    }

    let zugriffe = zugriffe.into_iter().collect::<BTreeSet<_>>();
    let result = grundbuchblaetter
        .intersection(&zugriffe)
        .cloned()
        .collect::<Vec<_>>();

    Ok(result)
}

pub fn get_benutzer_grouped_by_zugriff(
    ids: Vec<String>,
) -> Result<BTreeMap<String, BenutzerGrundbuecher>, String> {
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut map = BTreeMap::new();

    let mut stmt = conn
        .prepare(
            "SELECT name, typ, email, land, amtsgericht, bezirk, blatt FROM zugriffe WHERE id = ?1",
        )
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    for id in ids.iter() {
        let (name, rechte, email, land, amtsgericht, bezirk, blatt) = stmt
            .query_row(rusqlite::params![id], |r| {
                Ok((
                    r.get::<usize, String>(0)?,
                    r.get::<usize, String>(1)?,
                    r.get::<usize, String>(2)?,
                    r.get::<usize, String>(3)?,
                    r.get::<usize, String>(4)?,
                    r.get::<usize, String>(5)?,
                    r.get::<usize, String>(6)?,
                ))
            })
            .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

        let grundbuecher = map
            .entry(email)
            .or_insert_with(|| BenutzerGrundbuecher::default());

        grundbuecher.rechte = rechte.clone().replace("\"", "");
        grundbuecher.name = name.clone();
        grundbuecher.zugriff_id = id.clone();
        grundbuecher
            .grundbuecher
            .push((land, amtsgericht, bezirk, blatt));
        grundbuecher.grundbuecher.sort();
        grundbuecher.grundbuecher.dedup();
    }

    Ok(map)
}

pub fn zugriff_benutzer_exists(zugriff_id: &str) -> Result<bool, String> {
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let result =  conn.query_row(
        "SELECT COUNT(*) FROM benutzer WHERE password_hashed IS NOT NULL AND email = (SELECT email FROM zugriffe WHERE id = ?1 LIMIT 1)",
        rusqlite::params![zugriff_id],
        |r| match r.get::<usize, i32>(0)? {
            1 => Ok(true),
            _ => Ok(false),
        },
    ).map_err(|e| format!("{e}"))?;

    Ok(result)
}

pub fn create_zugriff(
    mount_point: MountPoint,
    id: &str,
    name: &str,
    email: &str,
    typ: ZugriffTyp,
    grund: &str,
    datum: &str,
    land: &str,
    amtsgericht: &str,
    bezirk: &str,
    blatt: &str,
) -> Result<(), String> {
    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "
        INSERT INTO zugriffe (
            id, name, email,
            typ, grund, land,
            amtsgericht, bezirk, blatt, angefragt
        ) VALUES (
            ?1, ?2, ?3, 
            ?4, ?5, ?6,
            ?7, ?8, ?9, ?10
        )",
        rusqlite::params![
            id,
            name,
            email,
            typ.to_string(),
            grund,
            land,
            amtsgericht,
            bezirk,
            format!("{blatt}"),
            datum
        ],
    )
    .map_err(|e| format!("Fehler beim Einfügen von Zugriff: {e}"))?;

    Ok(()) // TODO
}

pub fn generate_uuid() -> String {
    let token = uuid::Uuid::new_v4();
    format!("{token}")
}

pub fn generate_token() -> (String, DateTime<Utc>) {
    use uuid::Uuid;

    let gueltig_bis = Utc::now();
    let gueltig_bis = gueltig_bis
        .checked_add_signed(chrono::Duration::minutes(30))
        .unwrap_or(gueltig_bis);
    let token = Uuid::new_v4();
    let token = format!("{token}");

    (token, gueltig_bis)
}

pub fn check_password(
    mount_point: MountPoint,
    email: &str,
    passwort: &str,
) -> Result<(BenutzerInfo, String, DateTime<Utc>), Option<String>> {
    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| Some(format!("Fehler bei Verbindung zur Benutzerdatenbank")))?;

    let mut stmt = conn
        .prepare("SELECT id, name, email, rechte, password_hashed FROM benutzer WHERE email = ?1")
        .map_err(|e| Some(format!("Fehler beim Auslesen der Benutzerdaten 7")))?;

    let benutzer = stmt
        .query_map(rusqlite::params![email], |row| {
            Ok((
                row.get::<usize, i32>(0)?,
                row.get::<usize, String>(1)?,
                row.get::<usize, String>(2)?,
                row.get::<usize, String>(3)?,
                row.get::<usize, Option<Vec<u8>>>(4)?,
            ))
        })
        .map_err(|e| Some(format!("Fehler bei Verbindung zur Benutzerdatenbank")))?
        .collect::<Vec<_>>();

    let (id, name, email, rechte, pw) = match benutzer.get(0) {
        Some(Ok(s)) => s,
        _ => {
            return Err(Some(format!(
                "Kein Benutzerkonto für angegebene E-Mail-Adresse vorhanden"
            )));
        }
    };

    let pw = match pw {
        Some(s) => s,
        None => return Err(None), // noch kein Passwort gesetzt
    };

    if !verify_password(&pw, &passwort) {
        return Err(Some(format!("Ungültiges Passwort")));
    }

    let info = BenutzerInfo {
        id: *id,
        name: name.clone(),
        email: email.clone(),
        rechte: rechte.clone(),
    };

    let mut stmt = conn
        .prepare("SELECT token, gueltig_bis FROM sessions WHERE benutzer = ?1")
        .map_err(|e| Some(format!("Fehler beim Auslesen der Benutzerdaten 8")))?;

    let tokens = stmt
        .query_map(rusqlite::params![info.id], |row| {
            Ok((row.get::<usize, String>(0)?, row.get::<usize, String>(1)?))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();

    let now = Utc::now();

    for t in tokens {
        let t = t
            .as_ref()
            .ok()
            .and_then(|(t, g)| Some((t, DateTime::parse_from_rfc3339(&g).ok()?)));

        let (token, gueltig_bis) = match t {
            Some((t, g)) => (t, g),
            None => continue,
        };

        if !(now > gueltig_bis) {
            return Ok((info, token.clone(), gueltig_bis.into()));
        }
    }

    Err(None)
}

pub fn insert_token_into_sessions(
    mount_point: MountPoint,
    email: &str,
    token: &str,
    gueltig_bis: &DateTime<Utc>,
) -> Result<(), String> {
    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT id, name, email, rechte, password_hashed FROM benutzer WHERE email = ?1")
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten 9"))?;

    let benutzer = stmt
        .query_map(rusqlite::params![email], |row| {
            Ok((
                row.get::<usize, i32>(0)?,
                row.get::<usize, String>(1)?,
                row.get::<usize, String>(2)?,
                row.get::<usize, String>(3)?,
                row.get::<usize, Vec<u8>>(4)?,
            ))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();

    let (id, name, email, rechte, pw) = match benutzer.get(0) {
        Some(Ok(s)) => s,
        _ => {
            return Err(format!(
                "Kein Benutzerkonto für angegebene E-Mail-Adresse vorhanden"
            ));
        }
    };

    conn.execute(
        "INSERT INTO sessions (benutzer, token, gueltig_bis) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, token, gueltig_bis.to_rfc3339()],
    )
    .map_err(|e| format!("Fehler beim Einfügen von Token in Sessions: {e}"))?;

    Ok(())
}

pub fn get_public_key(email: &str, fingerprint: &str) -> Result<String, String> {
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT pubkey FROM publickeys WHERE email = ?1 AND fingerprint = ?2")
        .map_err(|e| format!("Fehler beim Auslesen der PublicKeys"))?;

    let keys = stmt
        .query_map([], |row| Ok(row.get::<usize, String>(0)?))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut bz = Vec::new();
    for b in keys {
        if let Ok(b) = b {
            bz.push(b);
        }
    }

    if bz.is_empty() {
        Err(format!(
            "Konnte keinen Schlüssel für {email} / {fingerprint} finden"
        ))
    } else {
        Ok(bz[0].clone())
    }
}

pub fn delete_user(mount_point: MountPoint, email: &str) -> Result<(), String> {
    let mut conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Fehler beim Löschen von Benutzer: {e}"))?;

    tx.execute(
        "DELETE FROM benutzer WHERE email = ?1",
        rusqlite::params![email],
    )
    .map_err(|e| format!("Fehler beim Löschen von Benutzer: {e}"))?;

    tx.execute(
        "DELETE FROM publickeys WHERE email = ?1",
        rusqlite::params![email],
    )
    .map_err(|e| format!("Fehler beim Löschen von Benutzer: {e}"))?;

    tx.commit()
        .map_err(|e| format!("Fehler beim Löschen von Benutzer: {e}"))?;

    Ok(())
}

pub fn create_abo(
    mount_point: MountPoint,
    typ: &str,
    blatt: &str,
    text: &str,
    aktenzeichen: Option<&str>,
) -> Result<(), String> {
    match typ {
        "email" | "webhook" => {}
        _ => {
            return Err(format!("Ungültiger Abonnement-Typ: {typ}"));
        }
    }

    let blatt_split = blatt
        .split("/")
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();

    let amtsgericht = match blatt_split.get(0) {
        Some(s) => s.trim().to_string(),
        None => {
            return Err(format!("Kein Amtsgericht angegeben für Abonnement {blatt}"));
        }
    };

    let bezirk = match blatt_split.get(1) {
        Some(s) => s.trim().to_string(),
        None => {
            return Err(format!("Kein Bezirk angegeben für Abonnement {blatt}"));
        }
    };

    let b = match blatt_split.get(2) {
        Some(s) => s
            .trim()
            .parse::<i32>()
            .map_err(|e| format!("Ungültige Blatt-Nr. {s}: {e}"))?,
        None => {
            return Err(format!("Kein Blatt angegeben für Abonnement {blatt}"));
        }
    };

    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "INSERT INTO abonnements (typ, text, amtsgericht, bezirk, blatt, aktenzeichen) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![typ, text, amtsgericht, bezirk, b, aktenzeichen.map(|s| s.to_string())],
    ).map_err(|e| format!("Fehler beim Einfügen von {blatt} in Abonnements: {e}"))?;

    Ok(())
}

pub fn get_abos_fuer_benutzer(benutzer: &BenutzerInfo) -> Result<Vec<AbonnementInfo>, String> {
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare(
            "SELECT typ, amtsgericht, bezirk, blatt, aktenzeichen FROM abonnements WHERE text = ?1",
        )
        .map_err(|e| format!("Fehler beim Auslesen der Abonnements"))?;

    let abos = stmt
        .query_map(rusqlite::params![benutzer.email], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, String>(1)?,
                row.get::<usize, String>(2)?,
                row.get::<usize, i32>(3)?,
                row.get::<usize, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut bz = Vec::new();

    for a in abos {
        if let Ok((typ, amtsgericht, bezirk, blatt, aktenzeichen)) = a {
            bz.push(AbonnementInfo {
                amtsgericht: amtsgericht.clone(),
                grundbuchbezirk: bezirk.clone(),
                blatt: blatt,
                text: benutzer.email.to_string(),
                aktenzeichen: aktenzeichen.as_ref().map(|s| s.to_string()),
            });
        }
    }

    Ok(bz)
}

pub fn get_email_abos(blatt: &str) -> Result<Vec<AbonnementInfo>, String> {
    get_abos_inner("email", blatt)
}

pub fn get_webhook_abos(blatt: &str) -> Result<Vec<AbonnementInfo>, String> {
    get_abos_inner("webhook", blatt)
}

fn get_abos_inner(typ: &'static str, blatt: &str) -> Result<Vec<AbonnementInfo>, String> {
    let blatt_split = blatt
        .split("/")
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();

    let amtsgericht = match blatt_split.get(0) {
        Some(s) => s.trim().to_string(),
        None => {
            return Err(format!("Kein Amtsgericht angegeben für Abonnement {blatt}"));
        }
    };

    let bezirk = match blatt_split.get(1) {
        Some(s) => s.trim().to_string(),
        None => {
            return Err(format!("Kein Bezirk angegeben für Abonnement {blatt}"));
        }
    };

    let b = match blatt_split.get(2) {
        Some(s) => s
            .trim()
            .parse::<i32>()
            .map_err(|e| format!("Ungültige Blatt-Nr. {s}: {e}"))?,
        None => {
            return Err(format!("Kein Blatt angegeben für Abonnement {blatt}"));
        }
    };

    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT text, aktenzeichen FROM abonnements WHERE typ = ?1 AND amtsgericht = ?2 AND bezirk = ?3 AND blatt = ?4")
        .map_err(|e| format!("Fehler beim Auslesen der Bezirke"))?;

    let abos = stmt
        .query_map(rusqlite::params![typ, amtsgericht, bezirk, b], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, Option<String>>(1)?,
            ))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut bz = Vec::new();

    for a in abos {
        if let Ok((email, aktenzeichen)) = a {
            bz.push(AbonnementInfo {
                amtsgericht: amtsgericht.clone(),
                grundbuchbezirk: bezirk.clone(),
                blatt: b.clone(),
                text: email.to_string(),
                aktenzeichen: aktenzeichen.as_ref().map(|s| s.to_string()),
            });
        }
    }

    Ok(bz)
}

pub fn delete_abo(
    mount_point: MountPoint,
    typ: &str,
    blatt: &str,
    text: &str,
    aktenzeichen: Option<&str>,
) -> Result<(), String> {
    match typ {
        "email" | "webhook" => {}
        _ => {
            return Err(format!("Ungültiger Abonnement-Typ: {typ}"));
        }
    }

    let blatt_split = blatt
        .split("/")
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();

    let amtsgericht = match blatt_split.get(0) {
        Some(s) => s.trim().to_string(),
        None => {
            return Err(format!("Kein Amtsgericht angegeben für Abonnement {blatt}"));
        }
    };

    let bezirk = match blatt_split.get(1) {
        Some(s) => s.trim().to_string(),
        None => {
            return Err(format!("Kein Bezirk angegeben für Abonnement {blatt}"));
        }
    };

    let b = match blatt_split.get(2) {
        Some(s) => s
            .trim()
            .parse::<i32>()
            .map_err(|e| format!("Ungültige Blatt-Nr. {s}: {e}"))?,
        None => {
            return Err(format!("Kein Blatt angegeben für Abonnement {blatt}"));
        }
    };

    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    match aktenzeichen.as_ref() {
        Some(s) => {
            conn.execute(
                "DELETE FROM abonnements WHERE text = ?1 AND amtsgericht = ?2 AND bezirk = ?3 AND blatt = ?4 AND aktenzeichen = ?5 AND typ = ?6",
                rusqlite::params![text, amtsgericht, bezirk, b, aktenzeichen, typ],
            ).map_err(|e| format!("Fehler beim Löschen von {blatt} in Abonnements: {e}"))?;
        }
        None => {
            conn.execute(
                "DELETE FROM abonnements WHERE text = ?1 AND amtsgericht = ?2 AND bezirk = ?3 AND blatt = ?4 AND typ = ?5",
                rusqlite::params![text, amtsgericht, bezirk, b, typ],
            ).map_err(|e| format!("Fehler beim Löschen von {blatt} in Abonnements: {e}"))?;
        }
    }

    Ok(())
}
