/// API für `/upload` Anfragen
pub mod upload {

    use crate::models::{AuthFormData, PdfFile};
    use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use url_encoded_data::UrlEncodedData;

    pub type FileName = String;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UploadChangeset {
        pub aenderung_titel: String,
        pub aenderung_beschreibung: String,
        pub neue_dateien: BTreeMap<FileName, PdfFile>,
        pub geaenderte_dateien: BTreeMap<FileName, GbxAenderung>,
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
        pub neu: BTreeMap<FileName, PdfFile>,
        pub geaendert: BTreeMap<FileName, PdfFile>,
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

        let folder_path = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("data");

        if !folder_path.exists() {
            let _ = std::fs::create_dir(folder_path.clone());
        }

        let mut check = UploadChangesetResponseOk {
            neu: BTreeMap::new(),
            geaendert: BTreeMap::new(),
        };

        let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();

        for (name, neu) in upload_changeset.neue_dateien.iter() {
            let amtsgericht = &neu.titelblatt.amtsgericht;
            let grundbuch = &neu.titelblatt.grundbuch_von;

            if !gemarkungen
                .iter()
                .any(|(land, ag, bezirk)| ag == amtsgericht && bezirk == grundbuch)
            {
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
            }

            let blatt = neu.titelblatt.blatt;
            let target_path = folder_path
                .clone()
                .join(amtsgericht)
                .join(grundbuch)
                .join(&format!("{grundbuch}_{blatt}.gbx"));
            
            let target_json = serde_json::to_string_pretty(&neu).unwrap_or_default();
            let target_folder = folder_path.clone().join(amtsgericht).join(grundbuch);

            let _ = std::fs::create_dir_all(&target_folder);
            let _ = std::fs::write(target_path, target_json.as_bytes());

            check.neu.insert(name.clone(), neu.clone());
        }

        for (name, geaendert) in upload_changeset.geaenderte_dateien.iter() {
            
            let amtsgericht = &geaendert.neu.titelblatt.amtsgericht;
            let grundbuch = &geaendert.neu.titelblatt.grundbuch_von;

            if !gemarkungen
                .iter()
                .any(|(land, ag, bezirk)| ag == amtsgericht && bezirk == grundbuch)
            {
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
            }

            let blatt = geaendert.neu.titelblatt.blatt;
            let target_path = folder_path
                .clone()
                .join(amtsgericht)
                .join(grundbuch)
                .join(&format!("{grundbuch}_{blatt}.gbx"));
            let target_json = serde_json::to_string_pretty(&geaendert.neu).unwrap_or_default();
            let target_folder = folder_path.clone().join(amtsgericht).join(grundbuch);

            let _ = std::fs::create_dir_all(&target_folder);
            let _ = std::fs::write(target_path, target_json.as_bytes());

            check.geaendert.insert(name.clone(), geaendert.neu.clone());
        }

        if !check.geaendert.is_empty() || !check.neu.is_empty() {
            use git2::Repository;

            let repo = match Repository::open(&folder_path) {
                Ok(repo) => repo,
                Err(_) => match Repository::init(&folder_path) {
                    Ok(repo) => repo,
                    Err(e) => {
                        return HttpResponse::Ok().content_type("application/json").body(
                            serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                                UploadChangesetResponseError {
                                    code: 1,
                                    text: format!("Konnte Änderungstext nicht speichern"),
                                },
                            ))
                            .unwrap_or_default(),
                        );
                    }
                },
            };

            let mut index = match repo.index() {
                Ok(o) => o,
                Err(e) => {
                    return HttpResponse::Ok().content_type("application/json").body(
                        serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                            UploadChangesetResponseError {
                                code: 2,
                                text: format!("Konnte Änderungstext nicht speichern"),
                            },
                        ))
                        .unwrap_or_default(),
                    );
                }
            };

            let _ = index.add_all(["*.gbx"].iter(), git2::IndexAddOption::DEFAULT, None);
            let _ = index.write();

            let signature = match git2::Signature::now(&benutzer.name, &benutzer.email) {
                Ok(o) => o,
                Err(e) => {
                    return HttpResponse::Ok().content_type("application/json").body(
                        serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                            UploadChangesetResponseError {
                                code: 4,
                                text: format!("Konnte Änderungstext nicht speichern"),
                            },
                        ))
                        .unwrap_or_default(),
                    );
                }
            };

            let msg = format!(
                "{}\r\n\r\n{}",
                upload_changeset.aenderung_titel.trim(),
                upload_changeset.aenderung_beschreibung
            );

            let id = match index.write_tree() {
                Ok(o) => o,
                Err(e) => {
                    return HttpResponse::Ok().content_type("application/json").body(
                        serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                            UploadChangesetResponseError {
                                code: 5,
                                text: format!("Konnte Änderungstext nicht speichern"),
                            },
                        ))
                        .unwrap_or_default(),
                    );
                }
            };

            let tree = match repo.find_tree(id) {
                Ok(o) => o,
                Err(e) => {
                    return HttpResponse::Ok().content_type("application/json").body(
                        serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                            UploadChangesetResponseError {
                                code: 6,
                                text: format!("Konnte Änderungstext nicht speichern"),
                            },
                        ))
                        .unwrap_or_default(),
                    );
                }
            };

            let parent = repo
                .head()
                .ok()
                .and_then(|c| c.target())
                .and_then(|head_target| repo.find_commit(head_target).ok());

            let parents = match parent.as_ref() {
                Some(s) => vec![s],
                None => Vec::new(),
            };

            let commit =
                match repo.commit(Some("HEAD"), &signature, &signature, &msg, &tree, &parents) {
                    Ok(o) => o,
                    Err(e) => {
                        return HttpResponse::Ok().content_type("application/json").body(
                            serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                                UploadChangesetResponseError {
                                    code: 9,
                                    text: format!("Konnte Änderungstext nicht speichern"),
                                },
                            ))
                            .unwrap_or_default(),
                        );
                    }
                };
        }

        HttpResponse::Ok().content_type("application/json").body(
            serde_json::to_string_pretty(&UploadChangesetResponse::StatusOk(check))
                .unwrap_or_default(),
        )
    }
}

/// API für `/download` Anfragen
pub mod download {

    use crate::models::{AuthFormData, PdfFile};
    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    use url_encoded_data::UrlEncodedData;

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
        let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();

        if !gemarkungen
            .iter()
            .any(|(land, ag, bezirk)| ag == amtsgericht && bezirk == grundbuch_von)
        {
            return HttpResponse::Ok()
            .content_type("application/json")
            .body(serde_json::to_string_pretty(&PdfFileOrEmpty::NichtVorhanden(PdfFileNichtVorhanden {
                code: 1,
                text: format!("Ungültiges Amtsgericht oder ungültige Gemarkung: {amtsgericht}/{grundbuch_von}"),
            })).unwrap_or_default());
        }

        let folder_path = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("data");

        let file: Option<PdfFile> = if amtsgericht == "*" {
            let mut result_file_path = None;
            if let Some(s) = std::fs::read_dir(&folder_path).ok() {
                for entry in s {
                    let entry = match entry.ok() {
                        Some(s) => s,
                        None => continue,
                    };

                    let path = entry.path();
                    let metadata = match std::fs::metadata(&path).ok() {
                        Some(s) => s,
                        None => continue,
                    };

                    if !metadata.is_dir() {
                        continue;
                    }

                    if path.ends_with(&grundbuch_von) {
                        result_file_path = Some(path.to_path_buf());
                        break;
                    }
                }
            }

            result_file_path.as_ref().and_then(|fp| {
                let file_path = fp.join(&format!("{grundbuch_von}_{blatt}.gbx"));
                std::fs::read_to_string(&file_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
            })
        } else {
            let file_path = folder_path
                .join(amtsgericht)
                .join(grundbuch_von)
                .join(&format!("{grundbuch_von}_{blatt}.gbx"));
            std::fs::read_to_string(&file_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
        };

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

    use crate::models::{AuthFormData, PdfFile, Titelblatt};
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
        
        let folder_path = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
            .to_str()
            .unwrap_or_default()
            .to_string();

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

/// API für `/aenderung` Anfragen
pub mod aenderung {}
