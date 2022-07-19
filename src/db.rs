//! Operationen über die Benutzer-Datenbank

use rusqlite::{Connection, OpenFlags};
use crate::{
    MountPoint,
    api::pull::{PullResponseError, PullResponse},
    models::{BenutzerInfo, AbonnementInfo, get_db_path}
};
use chrono::{DateTime, Utc};
use serde_derive::{Serialize, Deserialize};
use std::collections::BTreeMap;

pub type GemarkungsBezirke = Vec<(String, String, String)>;
const PASSWORD_LEN: usize = 128;

pub async fn pull_db() -> Result<(), PullResponseError> {
    
    let k8s = crate::k8s::is_running_in_k8s().await;
    if !k8s {
        return Ok(());
    }
    
    let k8s_peers = crate::k8s::k8s_get_peer_ips().await
    .map_err(|_| PullResponseError {
        code: 500, 
        text: "Kubernetes aktiv, konnte aber Pods nicht lesen (keine ClusterRole-Berechtigung?)".to_string()
    })?;

    for peer in k8s_peers.iter() {
                
        let client = reqwest::Client::new();
        let res = client.post(&format!("http://{}:8081/pull", peer.ip))
            .send()
            .await;
        
        let json = match res {
            Ok(o) => o.json::<PullResponse>().await,
            Err(e) => {
                return Err(PullResponseError {
                    code: 500,
                    text: format!("Pod {}:{} konnte nicht synchronisiert werden: {e}", peer.namespace, peer.name),
                });
            },
        };

        match json {
            Ok(PullResponse::StatusOk(_)) => { },
            Ok(PullResponse::StatusError(e)) => return Err(e),
            Err(e) => {
                return Err(PullResponseError {
                    code: 500,
                    text: format!("Pod {}:{} konnte nicht synchronisiert werden: {e}", peer.namespace, peer.name),
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
    
    let conn = Connection::open_with_flags(get_db_path(mount_point), open_flags)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS benutzer (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                email           VARCHAR(255) UNIQUE NOT NULL,
                name            VARCHAR(255) NOT NULL,
                rechte          VARCHAR(255) NOT NULL,
                password_hashed BLOB NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
                benutzer        INTEGER PRIMARY KEY AUTOINCREMENT,
                token           VARCHAR(1024) UNIQUE NOT NULL,
                gueltig_bis     VARCHAR(255) NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS bezirke (
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
            blatt            INTEGER NOT NULL,
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

    Ok(())
}

pub fn create_gemarkung(
    mount_point: MountPoint, 
    land: &str, 
    amtsgericht: &str, 
    bezirk: &str,
) -> Result<(), String> {

    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|_| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "INSERT INTO bezirke (land, amtsgericht, bezirk) VALUES (?1, ?2, ?3)",
        rusqlite::params![land, amtsgericht, bezirk],
    )
    .map_err(|e| {
        format!("Fehler beim Einfügen von {land}/{amtsgericht}/{bezirk} in Datenbank: {e}")
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

pub fn delete_gemarkung(
    mount_point: MountPoint, 
    land: &str, 
    amtsgericht: &str, 
    bezirk: &str,
) -> Result<(), String> {
    
    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "DELETE FROM bezirke WHERE land = ?1 AND amtsgericht = ?2 AND bezirk = ?3",
        rusqlite::params![land, amtsgericht, bezirk],
    )
    .map_err(|e| {
        format!("Fehler beim Löschen von {land}/{amtsgericht}/{bezirk} in Datenbank: {e}")
    })?;

    Ok(())
}

pub fn create_user(
    mount_point: MountPoint, 
    name: &str, 
    email: &str, 
    passwort: &str, 
    rechte: &str, 
    pubkey: Option<GpgPublicKeyPair>
) -> Result<(), String> {
    
    if passwort.len() > 50 {
        return Err(format!("Passwort zu lang"));
    }

    let password_hashed = hash_password(passwort).as_ref().to_vec();

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
            rusqlite::params![email, public_key.fingerprint, public_key.public.join("\r\n")],
        ).map_err(|e| format!("Fehler beim Einfügen von publickey für {email} in Datenbank: {e}"))?;    
    }

    Ok(())
}

fn hash_password(password: &str) -> [u8; PASSWORD_LEN] {
    use sodiumoxide::crypto::pwhash::argon2id13;
    use sodiumoxide::crypto::pwhash::argon2id13::HashedPassword;

    let default_pw = HashedPassword([0; PASSWORD_LEN]);

    if let Err(e) = sodiumoxide::init() {
        return default_pw.0.clone();
    }

    let hash = argon2id13::pwhash(
        password.as_bytes(),
        argon2id13::OPSLIMIT_INTERACTIVE,
        argon2id13::MEMLIMIT_INTERACTIVE,
    )
    .unwrap_or(default_pw);

    hash.0
}

fn verify_password(database_pw: &[u8], password: &str) -> bool {
    use sodiumoxide::crypto::pwhash::argon2id13;

    if let Err(_) = sodiumoxide::init() {
        return false;
    }

    let mut password_hash: [u8; PASSWORD_LEN] = [0; PASSWORD_LEN];

    if database_pw.len() != PASSWORD_LEN {
        return false;
    }

    unsafe {
        std::ptr::copy(
            database_pw.as_ptr(),
            password_hash.as_mut_ptr(),
            PASSWORD_LEN,
        );
    }

    match argon2id13::HashedPassword::from_slice(database_pw) {
        Some(hp) => argon2id13::pwhash_verify(&hp, password.as_bytes()),
        _ => false,
    }
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
    
    let bytes = key.as_tsk().armored().to_vec()
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;
        
    let secret_key = String::from_utf8(bytes)
    .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;
            
    let public_key_bytes = key.armored().to_vec()
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

pub fn get_key_for_fingerprint(fingerprint: &str, email: &str) -> Result<sequoia_openpgp::Cert, String> {
    
    use sequoia_openpgp::parse::PacketParser;
    use sequoia_openpgp::Cert;
    use sequoia_openpgp::parse::Parse;
    
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;
    
    let mut stmt = conn
        .prepare("SELECT pubkey FROM publickeys WHERE email = ?1 AND fingerprint = ?2")
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten"))?;
    
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
        
    let ppr = PacketParser::from_bytes(pubkey.as_bytes())
        .map_err(|e| format!("{e}"))?;
    
    let cert = Cert::try_from(ppr)
        .map_err(|e| format!("{e}"))?;
    
    Ok(cert)
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KontoData {
    pub data: BTreeMap<String, Vec<String>>,
}

pub fn get_konto_data(benutzer: &BenutzerInfo) -> Result<KontoData, String> {

    let mut data = KontoData::default();
    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    match benutzer.rechte.as_str() {
        "admin" => {

        },
        "bearbeiter" => {

        },
        _ => { },
    }

    Ok(data)
}

pub fn get_user_from_token(token: &str) -> Result<BenutzerInfo, String> {

    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT id, gueltig_bis FROM sessions WHERE token = ?1")
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten"))?;
    
    let tokens = stmt
        .query_map(rusqlite::params![token], |row| {
            Ok((
                row.get::<usize, i32>(0)?,
                row.get::<usize, String>(2)?,
            ))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();

    
    let result = tokens.get(0)
    .and_then(|t| t.as_ref().ok().and_then(|(id, g)| Some((id, DateTime::parse_from_rfc3339(&g).ok()?))));
    
    let id = match result {
        Some((id, gueltig_bis)) => {
            let now = Utc::now();
            if now > gueltig_bis {
                return Err(format!("Token abgelaufen"));
            }
            id
        },
        None => { return Err(format!("Ungültiges Token")); }
    };

    let mut stmt = conn
        .prepare("SELECT name, email, rechte FROM benutzer WHERE id = ?1")
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten"))?;
    
    let benutzer = stmt
    .query_map(rusqlite::params![id], |row| {
        Ok((
            row.get::<usize, String>(1)?,
            row.get::<usize, String>(2)?,
            row.get::<usize, String>(3)?,
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

pub fn generate_token() -> (String, DateTime<Utc>) {

    use uuid::Uuid;

    let gueltig_bis = Utc::now();
    let gueltig_bis = gueltig_bis.checked_add_signed(chrono::Duration::minutes(30)).unwrap_or(gueltig_bis);
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
        .map_err(|e| Some(format!("Fehler beim Auslesen der Benutzerdaten")))?;
    
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
        .map_err(|e| Some(format!("Fehler bei Verbindung zur Benutzerdatenbank")))?
        .collect::<Vec<_>>();

    let (id, name, email, rechte, pw) = match benutzer.get(0) {
        Some(Ok(s)) => s,
        _ => { return Err(Some(format!("Kein Benutzerkonto für angegebene E-Mail-Adresse vorhanden"))); },
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
        .prepare("SELECT token, gueltig_bis FROM sessions WHERE id = ?1")
        .map_err(|e| Some(format!("Fehler beim Auslesen der Benutzerdaten")))?;
    
    let tokens = stmt
        .query_map(rusqlite::params![info.id], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, String>(1)?,
            ))
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();

    match tokens.get(0).and_then(|t| t.as_ref().ok().and_then(|(t, g)| Some((t, DateTime::parse_from_rfc3339(&g).ok()?)))) {
        Some((token, gueltig_bis)) => Ok((info, token.clone(), gueltig_bis.into())),
        None => Err(None),
    }
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
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten"))?;
    
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
        _ => { return Err(format!("Kein Benutzerkonto für angegebene E-Mail-Adresse vorhanden")); },
    };

    conn.execute(
        "INSERT INTO sessions (id, token, gueltig_bis) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, token, gueltig_bis.to_rfc3339()],
    ).map_err(|e| format!("Fehler beim Einfügen von Token in Sessions: {e}"))?;
    
    Ok(())
}

pub fn get_public_key(email: &str, fingerprint: &str) -> Result<String, String> {

    let conn = Connection::open(get_db_path(MountPoint::Local))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT pubkey FROM publickeys WHERE email = ?1 AND fingerprint = ?2")
        .map_err(|e| format!("Fehler beim Auslesen der PublicKeys"))?;

    let keys = stmt
        .query_map([], |row| { Ok(row.get::<usize, String>(0)?) })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;
    
    let mut bz = Vec::new();
    for b in keys {
        if let Ok(b) = b {
            bz.push(b);
        }
    }
    
    if bz.is_empty() {
        Err(format!("Konnte keinen Schlüssel für {email} / {fingerprint} finden"))
    } else {
        Ok(bz[0].clone())
    }
}

pub fn delete_user(
    mount_point: MountPoint, 
    email: &str
) -> Result<(), String> {
   
    let conn = Connection::open(get_db_path(mount_point))
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "DELETE FROM benutzer WHERE email = ?1",
        rusqlite::params![email],
    )
    .map_err(|e| format!("Fehler beim Löschen von Benutzer: {e}"))?;
    
    conn.execute(
        "DELETE FROM publickeys WHERE email = ?1",
        rusqlite::params![email],
    )
    .map_err(|e| format!("Fehler beim Löschen von Benutzer: {e}"))?;

    Ok(())
}

pub fn create_abo(
    mount_point: MountPoint, 
    typ: &str, 
    blatt: &str, 
    text: &str, 
    aktenzeichen: Option<&str>
) -> Result<(), String> {
    
    match typ {
        "email" | "webhook" => { },
        _ => { return Err(format!("Ungültiger Abonnement-Typ: {typ}")); },
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
        .prepare("SELECT typ, amtsgericht, bezirk, blatt, aktenzeichen FROM abonnements WHERE text = ?1")
        .map_err(|e| format!("Fehler beim Auslesen der Abonnements"))?;
              
    let abos = stmt
        .query_map(rusqlite::params![benutzer.email], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, String>(1)?,
                row.get::<usize, String>(2)?,
                row.get::<usize, i32>(3)?,
                row.get::<usize, Option<String>>(4)?
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
                row.get::<usize, Option<String>>(1)?
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
    aktenzeichen: Option<&str>
) -> Result<(), String> {
    
    match typ {
        "email" | "webhook" => { },
        _ => { return Err(format!("Ungültiger Abonnement-Typ: {typ}")); },
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
        },
        None => {
            conn.execute(
                "DELETE FROM abonnements WHERE text = ?1 AND amtsgericht = ?2 AND bezirk = ?3 AND blatt = ?4 AND typ = ?5",
                rusqlite::params![text, amtsgericht, bezirk, b, typ],
            ).map_err(|e| format!("Fehler beim Löschen von {blatt} in Abonnements: {e}"))?;
        }
    }

    Ok(())
}
