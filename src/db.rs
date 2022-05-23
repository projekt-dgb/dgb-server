//! Operationen über die Benutzer-Datenbank

use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding},
    PaddingScheme, PublicKey, RsaPrivateKey, RsaPublicKey,
};
use rusqlite::{Connection, OpenFlags};
use crate::models::{BenutzerInfo, AbonnementInfo, get_db_path, get_keys_dir};

pub type GemarkungsBezirke = Vec<(String, String, String)>;

pub fn create_database() -> Result<(), rusqlite::Error> {

    let mut open_flags = OpenFlags::empty();

    open_flags.set(OpenFlags::SQLITE_OPEN_CREATE, true);
    open_flags.set(OpenFlags::SQLITE_OPEN_FULL_MUTEX, true);
    open_flags.set(OpenFlags::SQLITE_OPEN_NOFOLLOW, true);
    open_flags.set(OpenFlags::SQLITE_OPEN_SHARED_CACHE, true);
    open_flags.set(OpenFlags::SQLITE_OPEN_READ_WRITE, true);

    let conn = Connection::open_with_flags(get_db_path(), open_flags)?;

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
            aktenzeichen     VARCHAR(1023) NOT NULL
        )",
        [],
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS publickeys (
                email           VARCHAR(255) NOT NULL,
                pubkey          TEXT NOT NULL,
                fingerprint     VARCHAR(2048) NOT NULL,
                benutzt         INTEGER NOT NULL
        )",
        [],
    )?;

    Ok(())
}

pub fn create_gemarkung(land: &str, amtsgericht: &str, bezirk: &str) -> Result<(), String> {
    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

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
    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let mut stmt = conn
        .prepare("SELECT land, amtsgericht, bezirk FROM bezirke")
        .map_err(|e| format!("Fehler beim Auslesen der Bezirke"))?;

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

pub fn delete_gemarkung(land: &str, amtsgericht: &str, bezirk: &str) -> Result<(), String> {
    let conn = Connection::open(get_db_path())
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

pub fn create_user(name: &str, email: &str, passwort: &str, rechte: &str) -> Result<(), String> {
    if passwort.len() > 50 {
        return Err(format!("Passwort zu lang"));
    }

    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    let password_hashed = hash_password(passwort).as_ref().to_vec();

    let public_key = create_gpg_key(name, email)?;
    conn.execute(
        "INSERT INTO publickeys (email, fingerprint, pubkey, benutzt) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![email, public_key.0, public_key.1, 0],
    ).map_err(|e| format!("Fehler beim Einfügen von publickey für {email} in Datenbank: {e}"))?;
    
    conn.execute(
        "INSERT INTO benutzer (email, name, rechte, password_hashed) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![email, name, rechte, password_hashed],
    )
    .map_err(|e| format!("Fehler beim Einfügen von {email} in Datenbank: {e}"))?;

    Ok(())
}

const PASSWORD_LEN: usize = 128;

// Passwort -> Salted PW
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

fn create_gpg_key(name: &str, email: &str) -> Result<(String, String), String> {

    use std::path::Path;
    use sequoia_openpgp::serialize::SerializeInto;

    let key = crate::pgp::generate(&format!("{name} <{email}>"))
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {e}"))?;
    
    let bytes = key.as_tsk().armored().to_vec()
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;
        
    let secret_key = String::from_utf8(bytes)
    .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;
    
    let _ = std::fs::create_dir_all(get_keys_dir());
    
    std::fs::write(Path::new(&get_keys_dir()).join(&format!("{email}.gpg")), &secret_key)
        .map_err(|e| format!("Konnte GPG-Schlüssel nicht in /keys speichern: {key}: {e}"))?;
        
    let public_key_bytes = key.armored().to_vec()
        .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;
    
    let public_key_str = String::from_utf8(public_key_bytes)
    .map_err(|e| format!("Konnte kein Zertifikat generieren: {key}: {e}"))?;
    
    let fingerprint = key.fingerprint();

    Ok((fingerprint.to_string(), public_key_str))
}


pub fn get_key_for_fingerprint(fingerprint: &str, email: &str) -> Result<sequoia_openpgp::Cert, String> {
    
    use sequoia_openpgp::parse::PacketParser;
    use sequoia_openpgp::Cert;
    use sequoia_openpgp::parse::Parse;
    
    println!("opening connection to db: {}", get_db_path());
    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;
    
    let mut stmt = conn
        .prepare("SELECT pubkey FROM publickeys WHERE email = ?1 AND fingerprint = ?2")
        .map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten"))?;

    println!("stmt ok!");
    
    let pubkeys = stmt
        .query_map(rusqlite::params![email, fingerprint], |row| {
            Ok(row.get::<usize, String>(0)?)
        })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();

    println!("pubkeys ok!");

    let pubkey = match pubkeys.get(0) {
        Some(Ok(s)) => s,
        _ => return Err(format!("Kein öffentlicher Schlüssel für E-Mail {email:?} / Fingerprint {fingerprint:?} gefunden")),
    };
    
    println!("get_key_for_fingerprint: {email}:\r\n{pubkey}");
    
    let ppr = PacketParser::from_bytes(pubkey.as_bytes())
        .map_err(|e| format!("{e}"))?;
    
    let cert = Cert::try_from(ppr)
        .map_err(|e| format!("{e}"))?;
    
    println!("ok cert created!");
    
    Ok(cert)
}

pub fn validate_user(query_string: &str) -> Result<BenutzerInfo, String> {
    use url_encoded_data::UrlEncodedData;

    let auth = UrlEncodedData::parse_str(query_string);
    
    let email = auth
        .get_first("email")
        .map(|s| s.to_string())
        .ok_or(format!("Keine E-Mail Adresse angegeben"))?;
    
    let passwort = auth
        .get_first("passwort")
        .map(|s| s.to_string())
        .ok_or(format!("Kein Passwort angegeben"))?;
    
    let conn = Connection::open(get_db_path())
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
        _ => return Err(format!("Kein Benutzer für \"{email}\" gefunden")),
    };

    if !verify_password(&pw, &passwort) {
        return Err(format!("Ungültiges Passwort"));
    }

    Ok(BenutzerInfo {
        id: *id,
        name: name.clone(),
        email: email.clone(),
        rechte: rechte.clone(),
    })
}

pub fn get_public_key(email: &str, fingerprint: &str) -> Result<String, String> {
    let conn = Connection::open(get_db_path())
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

pub fn delete_user(email: &str) -> Result<(), String> {
    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "DELETE FROM benutzer WHERE email = ?1",
        rusqlite::params![email],
    )
    .map_err(|e| format!("Fehler beim Löschen von {email}: {e}"))?;
    
    conn.execute(
        "DELETE FROM publickeys WHERE email = ?1 AND benutzt = 0",
        rusqlite::params![email],
    )
    .map_err(|e| format!("Fehler beim Löschen von {email}: {e}"))?;

    Ok(())
}

pub fn create_abo(typ: &str, blatt: &str, text: &str, aktenzeichen: &str) -> Result<(), String> {
    
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

    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "INSERT INTO abonnements (typ, text, amtsgericht, bezirk, blatt, aktenzeichen) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![typ, text, amtsgericht, bezirk, b, aktenzeichen],
    ).map_err(|e| format!("Fehler beim Einfügen von {blatt} in Abonnements: {e}"))?;

    Ok(())
}

pub fn get_abos_fuer_benutzer(benutzer: &BenutzerInfo) -> Result<Vec<AbonnementInfo>, String> {
    
    let conn = Connection::open(get_db_path())
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
                row.get::<usize, String>(4)?
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
                aktenzeichen: aktenzeichen.to_string(),
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
    
    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;
        
    let mut stmt = conn
        .prepare("SELECT text, aktenzeichen FROM abonnements WHERE typ = ?1 AND amtsgericht = ?2 AND bezirk = ?3 AND blatt = ?4")
        .map_err(|e| format!("Fehler beim Auslesen der Bezirke"))?;
        
    let abos = stmt
        .query_map(rusqlite::params![typ, amtsgericht, bezirk, b], |row| {
            Ok((
                row.get::<usize, String>(0)?,
                row.get::<usize, String>(1)?
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
                aktenzeichen: aktenzeichen.to_string(),
            });
        }
    }
    
    Ok(bz)
}

pub fn delete_abo(typ: &str, blatt: &str, text: &str, aktenzeichen: &str) -> Result<(), String> {
    
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

    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "DELETE FROM abonnements WHERE text = ?1 AND amtsgericht = ?2 AND bezirk = ?3 AND blatt = ?4 AND aktenzeichen = ?5 AND typ = ?6",
        rusqlite::params![text, amtsgericht, bezirk, b, aktenzeichen, typ],
    ).map_err(|e| format!("Fehler beim Löschen von {blatt} in Abonnements: {e}"))?;

    Ok(())
}
