//! Operationen über die Benutzer-Datenbank

use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding},
    PaddingScheme, PublicKey, RsaPrivateKey, RsaPublicKey,
};
use rusqlite::{Connection, OpenFlags};

pub static DB_FILE_NAME: &str = "benutzer.sqlite.db";

pub type GemarkungsBezirke = Vec<(String, String, String)>;

fn get_db_path() -> String {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join(DB_FILE_NAME)
        .to_str()
        .unwrap_or_default()
        .to_string()
}

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
            email            VARCHAR(1023) NOT NULL,
            amtsgericht      VARCHAR(255) NOT NULL,
            bezirk           VARCHAR(255) NOT NULL,
            blatt            INTEGER NOT NULL,
            aktenzeichen     VARCHAR(1023) NOT NULL
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

fn create_gpg_key(name: &str, email: &str) -> Result<String, String> {
    Ok(String::new()) // TODO

    /*
        let mut rng = rand::thread_rng();
        let bits = 2048;

        let private_key = RsaPrivateKey::new(&mut rng, bits)
        .map_err(|e| format!("Fehler in RsaPrivateKey::new: {e}"))?;

        let public_key = RsaPublicKey::from(&private_key);



        private_key.write_pkcs8_pem_file(format!("./{email}.pem"), LineEnding::CRLF)
        .map_err(|e| format!("Fehler beim Schreiben von {email}.pem: {e}"))?;
    */

    /*

    let private_key = RsaPrivateKey::from_pkcs8_pem(&private_key)
    .map_err(|e| format!("Ungültiger privater Schlüssel"))?;

    let passwort_decrypted = private_key.decrypt(PaddingScheme::new_pkcs1v15_encrypt(), pw.as_ref())
        .map_err(|e| format!("Passwort falsch: Privater Schlüssel ungültig für {email}"))?;

    let passwort_decrypted = String::from_utf8(passwort_decrypted)
        .map_err(|e| format!("Konnte Passwort nicht entschlüsseln"))?;

    if passwort_decrypted != passwort {
        return Err(format!("Ungültiges Passwort"));
    }*/
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenutzerInfo {
    pub id: i32,
    pub rechte: String,
    pub name: String,
    pub email: String,
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

pub fn delete_user(email: &str) -> Result<(), String> {
    let conn = Connection::open(get_db_path())
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "DELETE FROM benutzer WHERE email = ?1",
        rusqlite::params![email],
    )
    .map_err(|e| format!("Fehler beim Löschen von {email}: {e}"))?;

    Ok(())
}

pub fn create_abo(blatt: &str, email: &str, aktenzeichen: &str) -> Result<(), String> {
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
        "INSERT INTO abonnements (email, amtsgericht, bezirk, blatt, aktenzeichen) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![email, amtsgericht, bezirk, b, aktenzeichen],
    ).map_err(|e| format!("Fehler beim Einfügen von {blatt} in Abonnements: {e}"))?;

    Ok(())
}

pub fn delete_abo(blatt: &str, email: &str, aktenzeichen: &str) -> Result<(), String> {
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
        "DELETE FROM abonnements WHERE email = ?1 AND amtsgericht = ?2 AND bezirk = ?3 AND blatt = ?4 AND aktenzeichen = ?5",
        rusqlite::params![email, amtsgericht, bezirk, b, aktenzeichen],
    ).map_err(|e| format!("Fehler beim Löschen von {blatt} in Abonnements: {e}"))?;

    Ok(())
}
