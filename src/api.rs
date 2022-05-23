/// API für `/upload` Anfragen
pub mod upload {

    use crate::models::{AuthFormData, PdfFile, BenutzerInfo, get_data_dir};
    use crate::db::GemarkungsBezirke;
    use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    use std::collections::{BTreeMap, BTreeSet};
    use url_encoded_data::UrlEncodedData;
    use std::path::PathBuf;
    
    pub type FileName = String;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UploadChangeset {
        pub titel: String,
        pub beschreibung: Vec<String>,
        pub fingerprint: String,
        pub signatur: PgpSignatur,
        pub data: UploadChangesetData,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PgpSignatur {
        pub hash: String,
        pub pgp_signatur: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UploadChangesetData {
        pub neu: Vec<PdfFile>,
        pub geaendert: Vec<GbxAenderung>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct GbxAenderung {
        pub alt: PdfFile,
        pub neu: PdfFile,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    pub enum UploadChangesetResponse {
        #[serde(rename = "ok")]
        StatusOk(UploadChangesetResponseOk),
        #[serde(rename = "error")]
        StatusError(UploadChangesetResponseError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UploadChangesetResponseOk {
        pub neu: Vec<PdfFile>,
        pub geaendert: Vec<PdfFile>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UploadChangesetResponseError {
        pub code: isize,
        pub text: String,
    }

    #[post("/upload")]
    async fn upload(
        upload_changeset: web::Json<UploadChangeset>,
        req: HttpRequest,
    ) -> impl Responder {
        
        use std::path::Path;
        
        let upload_changeset = &*upload_changeset;
        
        let benutzer = match crate::db::validate_user(&req.query_string()) {
            Ok(o) => o,
            Err(e) => {
                let json = serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                    UploadChangesetResponseError {
                        code: 0,
                        text: format!("Fehler bei Authentifizierung: {e}"),
                    },
                ))
                .unwrap_or_default();

                return HttpResponse::Ok()
                    .content_type("application/json")
                    .body(json);
            }
        };
        
        if let Err(e) = verify_signature(&benutzer.email, &upload_changeset) {
            return HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                    UploadChangesetResponseError {
                        code: 500,
                        text: format!("Fehler bei Überprüfung der digitalen Signatur:\r\n{e}"),
                    },
                ))
                .unwrap_or_default(),
            );
        }
        
        let folder_path = get_data_dir();
        let folder_path = Path::new(&folder_path);
        if !folder_path.exists() {
            let _ = std::fs::create_dir(folder_path.clone());
        }

        let mut check = UploadChangesetResponseOk {
            neu: Vec::new(),
            geaendert: Vec::new(),
        };

        let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();
        
        for neu in upload_changeset.data.neu.iter() {
        
            let amtsgericht = &neu.titelblatt.amtsgericht;
            let grundbuch = &neu.titelblatt.grundbuch_von;
            let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                if ag == amtsgericht && bezirk == grundbuch { Some(land.clone()) } else { None }
            });
            
            let land = match land {
                None => {
                    return HttpResponse::Ok().content_type("application/json").body(
                        serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                            UploadChangesetResponseError {
                                code: 1,
                                text: format!(
                                    "Ungültiges Amtsgericht oder ungültige Gemarkung: {}/{}",
                                    amtsgericht, grundbuch
                                ),
                            },
                        ))
                        .unwrap_or_default(),
                    );
                },
                Some(s) => s,
            };
            
            let blatt = neu.titelblatt.blatt;
            let target_path = folder_path
                .clone()
                .join(land.clone())
                .join(amtsgericht)
                .join(grundbuch)
                .join(&format!("{grundbuch}_{blatt}.gbx"));
                        
            let target_json = serde_json::to_string_pretty(&neu).unwrap_or_default();
            let target_folder = folder_path.clone()
                .join(land)
                .join(amtsgericht)
                .join(grundbuch);

            let _ = std::fs::create_dir_all(&target_folder);
            let _ = std::fs::write(target_path, target_json.as_bytes());

            check.neu.push(neu.clone());
        }

        for geaendert in upload_changeset.data.geaendert.iter() {
            
            let amtsgericht = &geaendert.neu.titelblatt.amtsgericht;
            let grundbuch = &geaendert.neu.titelblatt.grundbuch_von;

            let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                if ag == amtsgericht && bezirk == grundbuch { Some(land.clone()) } else { None }
            });
            
            let land = match land {
                None => {
                    return HttpResponse::Ok().content_type("application/json").body(
                        serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                            UploadChangesetResponseError {
                                code: 1,
                                text: format!(
                                    "Ungültiges Amtsgericht oder ungültige Gemarkung: {}/{}",
                                    amtsgericht, grundbuch
                                ),
                            },
                        ))
                        .unwrap_or_default(),
                    );
                },
                Some(s) => s,
            };

            let blatt = geaendert.neu.titelblatt.blatt;
            let target_path = folder_path
                .clone()
                .join(land.clone())
                .join(amtsgericht)
                .join(grundbuch)
                .join(&format!("{grundbuch}_{blatt}.gbx"));
            
            let target_json = serde_json::to_string_pretty(&geaendert.neu).unwrap_or_default();
            let target_folder = folder_path.clone()
                .join(land)
                .join(amtsgericht)
                .join(grundbuch);

            let _ = std::fs::create_dir_all(&target_folder);
            let _ = std::fs::write(target_path, target_json.as_bytes());

            check.geaendert.push(geaendert.neu.clone());
        }
        
        let server_url = "https://127.0.0.1"; // TODO
        
        if !check.geaendert.is_empty() || !check.neu.is_empty() {
            if let Err(e) = commit_changes(&gemarkungen, &server_url, &folder_path.to_path_buf(), &benutzer, &upload_changeset).await {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(
                    serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                        UploadChangesetResponseError {
                            code: 500,
                            text: format!("Konnte Änderungstext nicht speichern: {e}"),
                        },
                    ))
                    .unwrap_or_default(),
                );
            }
        }

        HttpResponse::Ok()
        .content_type("application/json")
        .body(
            serde_json::to_string_pretty(&UploadChangesetResponse::StatusOk(check))
                .unwrap_or_default(),
        )
    }
    
    fn verify_signature(email: &str, changeset: &UploadChangeset) -> Result<bool, String> {

        use sequoia_openpgp::policy::StandardPolicy as P;

        let json = serde_json::to_string_pretty(&changeset.data)
            .map_err(|e| format!("Konnte .data nicht zu JSON konvertieren: {e}"))?;
        let hash = &changeset.signatur.hash;
        let signatur = changeset.signatur.pgp_signatur.clone().join("\r\n");
        let msg = format!("-----BEGIN PGP SIGNED MESSAGE-----\r\nHash: {hash}\r\n\r\n{json}\r\n-----BEGIN PGP SIGNATURE-----\r\n{signatur}\r\n-----END PGP SIGNATURE-----");

        let p = &P::new();
        let cert = crate::db::get_key_for_fingerprint(&changeset.fingerprint, email)?;
        let mut plaintext = Vec::new();
        let _ = crate::pgp::verify(p, &mut plaintext, msg.as_bytes(), &cert)
            .map_err(|e| format!("{e}"))?;
        
        Ok(true)
    }
    
    fn commit_header_no_signature(
        commit_titel: &str,
        commit_beschreibung: &[String],
        fingerprint: &str,
    ) -> String {
        let mut target = String::new();
        target.push_str(commit_titel);
        target.push_str("\r\n\r\n");
        if !commit_beschreibung.is_empty() {
            target.push_str(&commit_beschreibung.to_vec().join("\r\n"));
            target.push_str("\r\n");        
        }
        target
    }
    
    fn commit_header_with_signature(
        commit_titel: &str,
        commit_beschreibung: &[String],
        fingerprint: &str,
        signatur: &PgpSignatur,
    ) -> String {
        let mut no_sig = commit_header_no_signature(commit_titel, commit_beschreibung, fingerprint);
        no_sig.push_str("\r\n");
        no_sig.push_str(&format!("Hash:         {}\r\n", signatur.hash));
        no_sig.push_str(&format!("Schlüssel-ID: {fingerprint}\r\n"));
        no_sig.push_str("\r\n");
        no_sig.push_str("-----BEGIN PGP SIGNATURE-----\r\n");
        no_sig.push_str(&signatur.pgp_signatur.to_vec().join("\r\n"));
        no_sig.push_str("\r\n-----END PGP SIGNATURE-----\r\n");
        no_sig
    }
    
    async fn commit_changes(
        gemarkungen: &GemarkungsBezirke,
        server_url: &str, 
        folder_path: &PathBuf, 
        benutzer: &BenutzerInfo, 
        upload_changeset: &UploadChangeset
    ) -> Result<(), String> {
    
        use git2::Repository;

        let repo = match Repository::open(&folder_path) {
            Ok(o) => o,
            Err(_) => { Repository::init(&folder_path).map_err(|e| format!("{e}"))? },
        };

        let mut index = repo.index().map_err(|e| format!("{e}"))?;
        let _ = index.add_all(["*.gbx"].iter(), git2::IndexAddOption::DEFAULT, None);
        let _ = index.write();

        let signature = git2::Signature::now(&benutzer.name, &benutzer.email)
            .map_err(|e| format!("{e}"))?;

        let msg = commit_header_with_signature(
            upload_changeset.titel.trim(),
            upload_changeset.beschreibung.as_ref(),
            upload_changeset.fingerprint.as_str(),
            &upload_changeset.signatur,
        );
                
        let id = index.write_tree().map_err(|e| format!("{e}"))?;
        let tree = repo.find_tree(id).map_err(|e| format!("{e}"))?;

        let parent = repo
            .head()
            .ok()
            .and_then(|c| c.target())
            .and_then(|head_target| repo.find_commit(head_target).ok());

        let parents = match parent.as_ref() {
            Some(s) => vec![s],
            None => Vec::new(),
        };

        let commit_id = repo
            .commit(Some("HEAD"), &signature, &signature, &msg, &tree, &parents)
            .map_err(|e| format!("{e}"))?;

        let commit_id = format!("{}", commit_id);
                
        let geaendert_blaetter = upload_changeset.data.geaendert.iter()
            .map(|aenderung| { 
                let tb = &aenderung.neu.titelblatt;  
                format!("{}/{}/{}", tb.amtsgericht, tb.grundbuch_von, tb.blatt)
            })
            .collect::<BTreeSet<_>>();
        
        let (grundbuch_schema, grundbuch_index) = crate::index::get_grundbuch_index()
        .map_err(|e| format!("Fehler in Index / Schema \"grundbuch\": {e}"))?;
        
        let mut index_writer = grundbuch_index.writer(10_000_000)
        .map_err(|e| format!("Fehler bei Allokation von 10MB für Schema \"grundbuch\": {e}"))?;

        for blatt in upload_changeset.data.neu.iter() {
                    
            let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                if *ag == blatt.titelblatt.amtsgericht && 
                   *bezirk == blatt.titelblatt.grundbuch_von { 
                    Some(land.clone()) 
                } else { 
                    None 
                }
            });

            let land = land.ok_or(format!(
                "Kein Land für Grundbuch {}_{}.gbx gefunden", 
                blatt.titelblatt.grundbuch_von, 
                blatt.titelblatt.blatt
            ))?;
                
            crate::index::add_grundbuchblatt_zu_index(&land, blatt, &index_writer, &grundbuch_schema)?;
        }
            
        for blatt in upload_changeset.data.geaendert.iter() {
            
            let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                if *ag == blatt.neu.titelblatt.amtsgericht && 
                *bezirk == blatt.neu.titelblatt.grundbuch_von { 
                    Some(land.clone()) 
                } else { 
                    None 
                }
            });

            let land = land.ok_or(format!(
                "Kein Land für Grundbuch {}_{}.gbx gefunden", 
                blatt.neu.titelblatt.grundbuch_von, 
                blatt.neu.titelblatt.blatt
            ))?;
            

            crate::index::add_grundbuchblatt_zu_index(&land, &blatt.neu, &index_writer, &grundbuch_schema)?;
        }
        
        let _ = index_writer.commit()
            .map_err(|e| format!("Fehler bei index.commit(): {e}"))?;
        
        for blatt in geaendert_blaetter {            
            
            let webhook_abos = crate::db::get_webhook_abos(&blatt, &commit_id)
                .map_err(|e| format!("{e}"))?;
            
            for abo_info in webhook_abos {
                let _ = crate::email::send_change_webhook(server_url, &abo_info).await;
            }
            
            let email_abos = crate::db::get_email_abos(&blatt, &commit_id)
                .map_err(|e| format!("{e}"))?;
            
            for abo_info in email_abos {
                let _ = crate::email::send_change_email(server_url, &abo_info);
            }
        }
                
        Ok(())
    }
}

/// API für `/download` Anfragen
pub mod download {

    use crate::models::{AuthFormData, PdfFile, get_data_dir};
    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    use url_encoded_data::UrlEncodedData;
    use std::path::Path;
    
    #[derive(Debug, Clone, Deserialize, Serialize)]
    #[serde(tag = "status")]
    pub enum PdfFileOrEmpty {
        #[serde(rename = "ok")]
        Pdf(PdfFile),
        #[serde(rename = "error")]
        NichtVorhanden(PdfFileNichtVorhanden),
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct PdfFileNichtVorhanden {
        pub code: usize,
        pub text: String,
    }

    #[get("/download/gbx/{amtsgericht}/{grundbuch_von}/{blatt}")]
    async fn download_gbx(
        path: web::Path<(String, String, usize)>,
        req: HttpRequest,
    ) -> impl Responder {
        let _benutzer = match crate::db::validate_user(&req.query_string()) {
            Ok(o) => o,
            Err(e) => {
                let json = serde_json::to_string_pretty(&PdfFileOrEmpty::NichtVorhanden(
                    PdfFileNichtVorhanden {
                        code: 0,
                        text: format!("Fehler bei Authentifizierung: {e}"),
                    },
                ))
                .unwrap_or_default();

                return HttpResponse::Ok()
                    .content_type("application/json")
                    .body(json);
            }
        };

        let (amtsgericht, grundbuch_von, blatt) = &*path;
        let mut amtsgericht = amtsgericht.clone();
        let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();

        let mut l = None;
        for (land, ag, bezirk) in gemarkungen.iter() {
            if (amtsgericht == "*" && bezirk == grundbuch_von) ||
               (ag.as_str() == amtsgericht.as_str() && bezirk == grundbuch_von) {
                amtsgericht = ag.clone();
                l = Some(land.clone());
                break;
            }
        }
        
        let land = match l {
            Some(s) => s,
            None => {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&PdfFileOrEmpty::NichtVorhanden(PdfFileNichtVorhanden {
                    code: 1,
                    text: format!("Ungültiges Amtsgericht oder ungültige Gemarkung: {amtsgericht}/{grundbuch_von}"),
                })).unwrap_or_default());
            }
        };

        let folder_path = get_data_dir();
        let folder_path = Path::new(&folder_path);
        
        let file_path = folder_path
                .join(land)
                .join(amtsgericht)
                .join(grundbuch_von)
                .join(&format!("{grundbuch_von}_{blatt}.gbx"));
            
        let file: Option<PdfFile> = std::fs::read_to_string(&file_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        let response_json = match file {
            Some(s) => PdfFileOrEmpty::Pdf(s),
            None => PdfFileOrEmpty::NichtVorhanden(PdfFileNichtVorhanden {
                code: 404,
                text: format!("Datei für {grundbuch_von}_{blatt}.gbx nicht gefunden"),
            }),
        };

        HttpResponse::Ok()
            .content_type("application/json")
            .body(serde_json::to_string_pretty(&response_json).unwrap_or_default())
    }

    #[get("/download/pdf/{amtsgericht}/{grundbuch_von}/{blatt}")]
    async fn dowload_pdf(
        path: web::Path<(String, String, usize)>,
        req: HttpRequest,
    ) -> impl Responder {
        let _benutzer = match crate::db::validate_user(&req.query_string()) {
            Ok(o) => o,
            Err(e) => {
                let json = serde_json::to_string_pretty(&PdfFileOrEmpty::NichtVorhanden(
                    PdfFileNichtVorhanden {
                        code: 0,
                        text: format!("Fehler bei Authentifizierung: {e}"),
                    },
                ))
                .unwrap_or_default();

                return HttpResponse::Ok()
                    .content_type("application/json")
                    .body(json);
            }
        };

        HttpResponse::Ok()
            .content_type("application/pdf")
            .body(generate_pdf())
    }

    fn generate_pdf() -> Vec<u8> {
        use printpdf::*;
        let (doc, page1, layer1) =
            PdfDocument::new("PDF_Document_title", Mm(247.0), Mm(210.0), "Layer 1");
        let (page2, layer1) = doc.add_page(Mm(10.0), Mm(250.0), "Page 2, Layer 1");
        doc.save_to_bytes().unwrap_or_default()
    }
}

/// API für `/suche` Anfragen
pub mod suche {

    use crate::models::{AuthFormData, PdfFile, Titelblatt, get_data_dir};
    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use regex::Regex;
    use serde_derive::{Deserialize, Serialize};
    use std::path::{Path, PathBuf};
    use url_encoded_data::UrlEncodedData;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    pub enum GrundbuchSucheResponse {
        #[serde(rename = "ok")]
        StatusOk(GrundbuchSucheOk),
        #[serde(rename = "error")]
        StatusErr(GrundbuchSucheError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct GrundbuchSucheOk {
        pub ergebnisse: Vec<GrundbuchSucheErgebnis>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct GrundbuchSucheErgebnis {
        pub titelblatt: Titelblatt,
        pub ergebnis_text: String,
        pub gefunden_text: String,
        pub download_id: String,
        pub score: isize,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct GrundbuchSucheError {
        pub code: usize,
        pub text: String,
    }

    lazy_static::lazy_static! {
        static ref RE: Regex = Regex::new("(\\w*)\\s*(\\d*)").unwrap();
        static ref RE_2: Regex = Regex::new("(\\w*)\\s*Blatt\\s*(\\d*)").unwrap();
    }

    #[get("/suche/{suchbegriff}")]
    async fn suche(suchbegriff: web::Path<String>, req: HttpRequest) -> impl Responder {
        
        let _benutzer = match crate::db::validate_user(&req.query_string()) {
            Ok(o) => o,
            Err(e) => {
                let json = serde_json::to_string_pretty(&GrundbuchSucheResponse::StatusErr(
                    GrundbuchSucheError {
                        code: 0,
                        text: e.clone(),
                    },
                ))
                .unwrap_or_default();

                return HttpResponse::Ok()
                    .content_type("application/json")
                    .body(json);
            }
        };
        
        let folder_path = get_data_dir();

        let mut ergebnisse = Vec::new();

        // /data
        if let Ok(e) = std::fs::read_dir(&folder_path) {
            for entry in e {
                let entry = match entry {
                    Ok(o) => o,
                    _ => continue,
                };
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                // /data/Prenzlau
                if let Ok(e) = std::fs::read_dir(&path) {
                    for entry in e {
                        let entry = match entry {
                            Ok(o) => o,
                            _ => continue,
                        };
                        let path = entry.path();
                        if !path.is_dir() {
                            continue;
                        }

                        // /data/Prenzlau/Ludwigsburg
                        if let Ok(mut e) = search_files(&path, &suchbegriff) {
                            ergebnisse.append(&mut e);
                        }
                    }
                }
            }
        }

        let json =
            serde_json::to_string_pretty(&GrundbuchSucheResponse::StatusOk(GrundbuchSucheOk {
                ergebnisse,
            }))
            .unwrap_or_default();

        HttpResponse::Ok()
            .content_type("application/json")
            .body(json)
    }

    fn search_files(
        dir: &PathBuf,
        suchbegriff: &str,
    ) -> Result<Vec<GrundbuchSucheErgebnis>, std::io::Error> {
        use std::fs;

        let mut ergebnisse = Vec::new();

        if let Some(s) = normalize_search(suchbegriff) {
            if let Some(file) = fs::read_to_string(Path::new(dir).join(format!("{s}.gbx")))
                .ok()
                .and_then(|s| serde_json::from_str::<PdfFile>(&s).ok())
            {
                ergebnisse.push(GrundbuchSucheErgebnis {
                    titelblatt: file.titelblatt.clone(),
                    ergebnis_text: "".to_string(),
                    gefunden_text: "".to_string(),
                    download_id: format!(
                        "{}/{}/{}.gbx",
                        file.titelblatt.amtsgericht,
                        file.titelblatt.grundbuch_von,
                        file.titelblatt.blatt
                    ),
                    score: isize::MAX,
                });
            }
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::metadata(&path)?;

            if !metadata.is_file() {
                continue;
            }

            let file: Option<PdfFile> = std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok());

            let pdf = match file {
                Some(s) => s,
                None => continue,
            };

            let grundbuch_json = serde_json::to_string_pretty(&pdf.analysiert).unwrap_or_default();
            let lines = grundbuch_json.lines().collect::<Vec<_>>();

            for (i, line) in lines.iter().enumerate() {
                match sublime_fuzzy::best_match(suchbegriff, &line) {
                    Some(m) => {
                        let score = m.score();
                        ergebnisse.push(GrundbuchSucheErgebnis {
                            titelblatt: pdf.titelblatt.clone(),
                            ergebnis_text: {
                                let mut t = if i > 0 {
                                    format!(
                                        "{}\r\n<br/>",
                                        lines.get(i - 1).cloned().unwrap_or_default()
                                    )
                                } else {
                                    String::new()
                                };
                                t.push_str(&format!("{line}\r\n"));
                                if let Some(next) = lines.get(i + 1) {
                                    t.push_str(&format!("<br/>{next}\r\n"));
                                }
                                t
                            },
                            gefunden_text: suchbegriff.to_string(),
                            download_id: format!(
                                "{}/{}/{}.gbx",
                                pdf.titelblatt.amtsgericht,
                                pdf.titelblatt.grundbuch_von,
                                pdf.titelblatt.blatt
                            ),
                            score: score,
                        });
                        break;
                    }
                    None => continue,
                }
            }
        }

        ergebnisse.sort_by(|a, b| b.score.cmp(&a.score));
        ergebnisse.dedup();

        Ok(ergebnisse.into_iter().take(50).collect())
    }

    fn normalize_search(text: &str) -> Option<String> {
        if RE_2.is_match(text) {
            let grundbuch_von = RE_2
                .captures_iter(text)
                .nth(0)
                .and_then(|cap| Some(cap.get(1)?.as_str().to_string()))?;
            let blatt = RE_2
                .captures_iter(text)
                .nth(0)
                .and_then(|cap| Some(cap.get(2)?.as_str().parse::<usize>().ok()?))?;
            Some(format!("{grundbuch_von}_{blatt}"))
        } else if RE.is_match(text) {
            let grundbuch_von = RE
                .captures_iter(text)
                .nth(0)
                .and_then(|cap| Some(cap.get(1)?.as_str().to_string()))?;
            let blatt = RE
                .captures_iter(text)
                .nth(0)
                .and_then(|cap| Some(cap.get(2)?.as_str().parse::<usize>().ok()?))?;
            Some(format!("{grundbuch_von}_{blatt}"))
        } else {
            None
        }
    }
}

/// API für `/abo` Anfragen
pub mod abo {

    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum AboNeuAnfrage {
        #[serde(rename = "ok")]
        Ok(AboNeuAnfrageOk),
        #[serde(rename = "error")]
        Err(AboNeuAnfrageErr),
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AboNeuAnfrageOk { }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AboNeuAnfrageErr {
        code: usize,
        text: String,
    }

    #[get("/abo-neu/{email_oder_webhook}/{amtsgericht}/{grundbuchbezirk}/{blatt}/{tag}")]
    async fn abo_neu(tag: web::Path<(String, String, String, usize, String)>, req: HttpRequest) -> impl Responder {
        
        let benutzer = match crate::db::validate_user(&req.query_string()) {
            Ok(o) => o,
            Err(e) => {
                let json = serde_json::to_string_pretty(&AboNeuAnfrage::Err(
                    AboNeuAnfrageErr {
                        code: 0,
                        text: e.clone(),
                    },
                ))
                .unwrap_or_default();

                return HttpResponse::Ok()
                    .content_type("application/json")
                    .body(json);
            }
        };
    
        let (email_oder_webhook, amtsgericht, grundbuchbezirk, blatt, tag) = &*tag;
        
        if let Err(e) = crate::db::create_abo(&email_oder_webhook, &format!("{amtsgericht}/{grundbuchbezirk}/{blatt}"), &benutzer.email, &tag) {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&AboNeuAnfrage::Err(AboNeuAnfrageErr {
                    code: 0,
                    text: format!("{e}"),
                }))
                .unwrap_or_default());
        }
        
        return HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string_pretty(&AboNeuAnfrage::Ok(AboNeuAnfrageOk { }))
        .unwrap_or_default());
    }
    
    #[get("/abo-loeschen/{email_oder_webhook}/{amtsgericht}/{grundbuchbezirk}/{blatt}/{tag}")]
    async fn abo_loeschen(tag: web::Path<(String, String, String, usize, String)>, req: HttpRequest) -> impl Responder {
        
        let benutzer = match crate::db::validate_user(&req.query_string()) {
            Ok(o) => o,
            Err(e) => {
                let json = serde_json::to_string_pretty(&AboNeuAnfrage::Err(
                    AboNeuAnfrageErr {
                        code: 0,
                        text: e.clone(),
                    },
                ))
                .unwrap_or_default();

                return HttpResponse::Ok()
                    .content_type("application/json")
                    .body(json);
            }
        };
    
        let (email_oder_webhook, amtsgericht, grundbuchbezirk, blatt, tag) = &*tag;
        
        if let Err(e) = crate::db::delete_abo(&email_oder_webhook, &format!("{amtsgericht}/{grundbuchbezirk}/{blatt}"), &benutzer.email, &tag) {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&AboNeuAnfrage::Err(AboNeuAnfrageErr {
                    code: 0,
                    text: format!("{e}"),
                }))
                .unwrap_or_default());
        }
        
        return HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string_pretty(&AboNeuAnfrage::Ok(AboNeuAnfrageOk { }))
        .unwrap_or_default());
    }
}
