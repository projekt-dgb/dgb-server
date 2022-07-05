use actix_web::{HttpRequest, HttpResponse};
use crate::models::BenutzerInfo;

async fn get_benutzer_from_httpauth(req: &HttpRequest) -> Result<(String, BenutzerInfo), HttpResponse> {
    use self::upload::{UploadChangesetResponse, UploadChangesetResponseError};
    get_benutzer_from_httpauth_inner(req).await
    .map_err(|e| {
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
    })
}

async fn get_benutzer_from_httpauth_inner(req: &HttpRequest) -> Result<(String, BenutzerInfo), String> {
    use actix_web_httpauth::extractors::bearer::BearerAuth;
    use actix_web::FromRequest;

    let bearer = BearerAuth::extract(req).await
    .map_err(|e| format!("{e}"))?;

    let token = bearer.token();
    let user = crate::db::get_user_from_token(token)?;
    
    Ok((token.to_string(), user))
}

/// API für `/status` Anfragen
pub mod status {
    use actix_web::{get, post, HttpRequest, Responder, HttpResponse};
    
    // Startseite
    #[get("/")]
    async fn status(req: HttpRequest) -> impl Responder {
        let css = include_str!("../web/style.css");
        let css = format!("<style type='text/css'>{css}</style>");
        HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../web/index.html").replace("<!-- CSS -->", &css))
    }

    // Seite mit API-Dokumentation
    #[get("/api")]
    async fn api(req: HttpRequest) -> impl Responder {
        use comrak::{markdown_to_html, ComrakOptions};
        let html = markdown_to_html(include_str!("../API.md"), &ComrakOptions::default());
        let css = concat!(
            include_str!("../web/github-markdown-light.css"), 
            include_str!("../web/style.css")
        );
        let body = format!("
            <!DOCTYPE html>
            <html>
                <head><style>{css}</style></head>
                <body>
                <nav>
                    <ul>
                        <li>
                            <a href='/'><span>Startseite</span></a>
                            <a href='/konto'><span>Mein Konto</span></a>
                        </li>
                    </ul>
                </nav>
                <div class='readme'>
                {html}
                </div>
                </body>
            </html>
        ");
        
        HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
    }
}

pub mod login {

    use actix_web::{get, post, web, HttpRequest, Responder, HttpResponse};
    use serde_derive::{Serialize, Deserialize};
    use chrono::{DateTime, Utc};

    // Login-Seite
    #[get("/login")]
    async fn login_get(req: HttpRequest) -> impl Responder {
        let css = include_str!("../web/style.css");
        let css = format!("<style type='text/css'>{css}</style>");
        HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../web/login.html").replace("<!-- CSS -->", &css))
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct LoginForm {
        email: String,
        passwort: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum LoginResponse {
        Ok(LoginResponseOk),
        Error(LoginResponseError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct LoginResponseOk {
        token: String,
        valid_until: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct LoginResponseError {
        code: usize,
        text: String,
    }

    // Login-Seite
    #[post("/login")]
    async fn login_post(form: web::Form<LoginForm>, req: HttpRequest) -> impl Responder {
        
        let response = match crate::db::create_login_user_token(&form.email, &form.passwort) {
            Ok((_info, token, valid_until)) => LoginResponse::Ok(LoginResponseOk {
                token,
                valid_until, 
            }),
            Err(e) => {
                LoginResponse::Error(LoginResponseError {
                    code: 0,
                    text: e.clone(),
                })
            }
        };

        HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(serde_json::to_string_pretty(&response).unwrap_or_default())
    }
}

pub mod konto {
    use actix_web::{get, post, HttpRequest, Responder, HttpResponse};
    use serde_derive::{Serialize, Deserialize};

    // Konto-Seite
    #[get("/konto")]
    async fn konto_get(req: HttpRequest) -> impl Responder {

        let (token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(_) => { 
                return HttpResponse::Found()
                .append_header(("Location", "/login"))
                .finish(); 
            },
        };

        let konto_data = match crate::db::get_konto_data(&benutzer) {
            Ok(b) => b,
            Err(_) => {
                return HttpResponse::Found()
                    .append_header(("Location", "/login"))
                    .finish();
            }
        };

        let konto_data_json = serde_json::to_string(&konto_data).unwrap_or_default();
        let html = include_str!("../web/konto.html")
        .replace("<!-- CSS -->", &format!("<style>{}</style>", include_str!("../web/style.css")))
        .replace("data-konto-daten=\"{}\"", &format!("data-konto-daten=\'{}\'", konto_data_json))
        .replace("data-token-id=\"\"", &format!("data-token-id=\"{}\"", token));

        HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct KontoJsonPost {
        pub tabelle: String,
        #[serde(default)]
        pub ids: Vec<usize>,
        pub action: String,
    }

    // Login-Seite
    #[post("/konto")]
    async fn konto_post(req: HttpRequest) -> impl Responder {
        HttpResponse::Ok()
    }
}

/// API für `/k8s` Anfragen: Gibt eine Status-Übersicht des k8s-clusters aus
pub mod k8s {
    use actix_web::{get, HttpRequest, Responder, HttpResponse};
    
    // Test, um k8s-status abzufragen
    #[get("/k8s")]
    async fn k8s(req: HttpRequest) -> impl Responder {

        let body = match crate::k8s::k8s_get_peer_ips().await {
            Ok(peers) => peers.join("\r\n"),
            Err(e) => format!("{e}"),
        };

        let root_token = std::env::var("ROOT_TOKEN").ok();
        let root_gueltig_bis = std::env::var("ROOT_GUELTIG_BIS").ok();
        let root_email = std::env::var("ROOT_EMAIL").ok();
        let root_passwort = std::env::var("ROOT_PASSWORT").ok();

        let k8s_podlist = match crate::k8s::k8s_list_pods().await {
            Ok(o) => format!("OK: {o}"),
            Err(o) => format!("ERROR: {o}"),
        };

        let body = if crate::k8s::is_running_in_k8s().await {
            format!("k8s available:\r\n\r\npeers:\r\n{body}")    
        } else {
            format!("k8s not available")
        };

        let body = format!("
k8s_podlist: {k8s_podlist}

root_token:{root_token:?}
root_gueltig_bis:{root_gueltig_bis:?}
root_email:{root_email:?}
root_passwort:{root_passwort:?}

{body}");

        HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(body)
    }
}

/// API für `/upload` Anfragen
pub mod upload {

    use crate::models::{PdfFile, BenutzerInfo, get_data_dir};
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
        let (_token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => { return e; },
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
        
            let amtsgericht = &neu.analysiert.titelblatt.amtsgericht;
            let grundbuch = &neu.analysiert.titelblatt.grundbuch_von;
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
            
            let blatt = neu.analysiert.titelblatt.blatt;
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
            
            let amtsgericht = &geaendert.neu.analysiert.titelblatt.amtsgericht;
            let grundbuch = &geaendert.neu.analysiert.titelblatt.grundbuch_von;

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

            let blatt = geaendert.neu.analysiert.titelblatt.blatt;
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
                            code: 501,
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
            .map_err(|e| format!("Konnte .data nicht zu JSON konvertieren: {e}"))?
            .lines()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\r\n");
            
        let hash = &changeset.signatur.hash;
        let signatur = changeset.signatur.pgp_signatur.clone().join("\r\n");
        let msg = format!("-----BEGIN PGP SIGNED MESSAGE-----\r\nHash: {hash}\r\n\r\n{json}\r\n-----BEGIN PGP SIGNATURE-----\r\n{signatur}\r\n-----END PGP SIGNATURE-----");

        println!("msg received:\r\n{msg}");
        
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
                let tb = &aenderung.neu.analysiert.titelblatt;  
                format!("{}/{}/{}", tb.amtsgericht, tb.grundbuch_von, tb.blatt)
            })
            .collect::<BTreeSet<_>>();
        
        let (grundbuch_schema, grundbuch_index) = crate::index::get_grundbuch_index()
        .map_err(|e| format!("Fehler in Index / Schema \"grundbuch\": {e}"))?;
        
        let mut index_writer = grundbuch_index.writer(10_000_000)
        .map_err(|e| format!("Fehler bei Allokation von 10MB für Schema \"grundbuch\": {e}"))?;

        for blatt in upload_changeset.data.neu.iter() {
                    
            let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                if *ag == blatt.analysiert.titelblatt.amtsgericht && 
                   *bezirk == blatt.analysiert.titelblatt.grundbuch_von { 
                    Some(land.clone()) 
                } else { 
                    None 
                }
            });

            let land = land.ok_or(format!(
                "Kein Land für Grundbuch {}_{}.gbx gefunden", 
                blatt.analysiert.titelblatt.grundbuch_von, 
                blatt.analysiert.titelblatt.blatt
            ))?;
                
            crate::index::add_grundbuchblatt_zu_index(&land, blatt, &index_writer, &grundbuch_schema)?;
        }
            
        for blatt in upload_changeset.data.geaendert.iter() {
            
            let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                if *ag == blatt.neu.analysiert.titelblatt.amtsgericht && 
                *bezirk == blatt.neu.analysiert.titelblatt.grundbuch_von { 
                    Some(land.clone()) 
                } else { 
                    None 
                }
            });

            let land = land.ok_or(format!(
                "Kein Land für Grundbuch {}_{}.gbx gefunden", 
                blatt.neu.analysiert.titelblatt.grundbuch_von, 
                blatt.neu.analysiert.titelblatt.blatt
            ))?;
            

            crate::index::add_grundbuchblatt_zu_index(&land, &blatt.neu, &index_writer, &grundbuch_schema)?;
        }
        
        let _ = index_writer.commit()
            .map_err(|e| format!("Fehler bei index.commit(): {e}"))?;
        
        for blatt in geaendert_blaetter {            
            
            let webhook_abos = crate::db::get_webhook_abos(&blatt)
                .map_err(|e| format!("{e}"))?;
            
            for abo_info in webhook_abos {
                let _ = crate::email::send_change_webhook(server_url, &abo_info, &commit_id).await;
            }
            
            let email_abos = crate::db::get_email_abos(&blatt)
                .map_err(|e| format!("{e}"))?;
            
            for abo_info in email_abos {
                let _ = crate::email::send_change_email(server_url, &abo_info, &commit_id);
            }
        }
                
        Ok(())
    }
}

/// API für `/download` Anfragen
pub mod download {

    use crate::models::{PdfFile, get_data_dir};
    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    use url_encoded_data::UrlEncodedData;
    use std::path::Path;
    use crate::pdf::PdfGrundbuchOptions;
    use crate::models::Grundbuch;
    
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
        
        let (_token, _benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => { return e; },
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
        
        let (_token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => { return e; },
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

    
        let gbx = match file {
            Some(s) => s,
            None => {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&PdfFileOrEmpty::NichtVorhanden(PdfFileNichtVorhanden {
                    code: 404,
                    text: format!("Datei für {grundbuch_von}_{blatt}.gbx nicht gefunden"),
                })).unwrap_or_default());
            }
        };
        
        let options = PdfGrundbuchOptions {
            exportiere_bv: true,
            exportiere_abt1: true,
            exportiere_abt2: true,
            exportiere_abt3: true,
            leere_seite_nach_titelblatt: true,
            mit_geroeteten_eintraegen: true, // TODO
        };
        
        let pdf_bytes = generate_pdf(&gbx.analysiert, &options);
        
        HttpResponse::Ok()
            .content_type("application/pdf")
            .body(pdf_bytes)
    }

    fn generate_pdf(gb: &Grundbuch, options: &PdfGrundbuchOptions) -> Vec<u8> {

        use printpdf::Mm;
        use crate::pdf::PdfFonts;
        use crate::models::Grundbuch;
        use printpdf::PdfDocument;
        
        let grundbuch_von = gb.titelblatt.grundbuch_von.clone();
        let blatt = gb.titelblatt.blatt;
        let amtsgericht = gb.titelblatt.amtsgericht.clone();

        let titel = format!("{grundbuch_von} Blatt {blatt} (Amtsgericht {amtsgericht})");
        let (mut doc, page1, layer1) = PdfDocument::new(&titel, Mm(210.0), Mm(297.0), "Titelblatt");
        let titelblatt = format!("{}_{}", gb.titelblatt.grundbuch_von, gb.titelblatt.blatt);
        let fonts = PdfFonts::new(&mut doc);
        
        crate::pdf::write_titelblatt(&mut doc.get_page(page1).get_layer(layer1), &fonts, &gb.titelblatt);
        if options.leere_seite_nach_titelblatt {
            // Leere Seite 2
            let (_, _) = doc.add_page(Mm(210.0), Mm(297.0), "Formular");
        }
        
        crate::pdf::write_grundbuch(&mut doc, &gb, &fonts, &options);
        
        let bytes = doc.save_to_bytes().unwrap_or_default();
        bytes
    }
}

/// API für `/suche` Anfragen
pub mod suche {

    use crate::models::{AbonnementInfo, PdfFile, Titelblatt, get_data_dir};
    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use regex::Regex;
    use serde_derive::{Deserialize, Serialize};
    use std::path::{Path, PathBuf};
    use url_encoded_data::UrlEncodedData;
    use crate::suche::{SuchErgebnisAenderung, SuchErgebnisGrundbuch};
    
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
        pub grundbuecher: Vec<GrundbuchSucheErgebnis>,
        pub aenderungen: Vec<CommitSucheErgebnis>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct GrundbuchSucheErgebnis {
        pub titelblatt: Titelblatt,
        pub ergebnis: SuchErgebnisGrundbuch,
        pub abos: Vec<AbonnementInfo>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct CommitSucheErgebnis {
        pub aenderung_id: String,
        pub ergebnis: SuchErgebnisAenderung,
        pub titelblaetter: Vec<Titelblatt>,
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
        
        let (_token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => { return e; },
        };
        let folder_path = get_data_dir();
        let suchbegriff = &*suchbegriff;
        
        let ergebnisse = match crate::suche::suche_in_index(&suchbegriff) {
            Ok(o) => o,
            Err(e) => {
                let json = serde_json::to_string_pretty(&GrundbuchSucheResponse::StatusErr(
                    GrundbuchSucheError {
                        code: 500,
                        text: e.clone(),
                    },
                ))
                .unwrap_or_default();

                return HttpResponse::Ok()
                    .content_type("application/json")
                    .body(json);
            }
        };
        
        let abos = crate::db::get_abos_fuer_benutzer(&benutzer)
            .unwrap_or_default();
                
        let grundbuecher = ergebnisse.grundbuecher
            .into_iter()
            .filter_map(|ergebnis| {

                let titelblatt = Titelblatt {
                    amtsgericht: ergebnis.amtsgericht.clone(),
                    grundbuch_von: ergebnis.grundbuch_von.clone(),
                    blatt: ergebnis.blatt.parse().ok()?,
                };
                
                let abos = abos.iter().filter(|a| {
                    a.amtsgericht == ergebnis.amtsgericht &&
                    a.grundbuchbezirk == ergebnis.grundbuch_von &&
                    a.blatt.to_string() == ergebnis.blatt
                })
                .cloned()
                .collect();
                
                Some(GrundbuchSucheErgebnis {
                    titelblatt,
                    ergebnis,
                    abos,
                })
            })
            .collect::<Vec<_>>();
        
        let json =
            serde_json::to_string_pretty(&GrundbuchSucheResponse::StatusOk(GrundbuchSucheOk {
                grundbuecher: grundbuecher,
                aenderungen: Vec::new(),
            }))
            .unwrap_or_default();

        HttpResponse::Ok()
            .content_type("application/json")
            .body(json)
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

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AboNeuForm {
        tag: Option<String>
    }

    #[get("/abo-neu/{email_oder_webhook}/{amtsgericht}/{grundbuchbezirk}/{blatt}")]
    async fn abo_neu(
        path: web::Path<(String, String, String, usize)>, 
        form: web::Json<AboNeuForm>, 
        req: HttpRequest,
    ) -> impl Responder {
        
        let (_token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => { return e; },
        };
        let (email_oder_webhook, amtsgericht, grundbuchbezirk, blatt) = &*path;
        let abo = crate::db::create_abo(
            &email_oder_webhook, 
            &format!("{amtsgericht}/{grundbuchbezirk}/{blatt}"), 
            &benutzer.email, 
            form.tag.as_ref().map(|s| s.as_str())
        );

        let json = match abo {
            Ok(()) => {
                serde_json::to_string_pretty(&AboNeuAnfrage::Ok(AboNeuAnfrageOk { })).unwrap_or_default()
            },
            Err(e) => {
                serde_json::to_string_pretty(&AboNeuAnfrage::Err(AboNeuAnfrageErr {
                    code: 500,
                    text: format!("{e}"),
                })).unwrap_or_default()
            }
        };

        HttpResponse::Ok()
            .content_type("application/json")
            .body(json)
    }
    
    #[get("/abo-loeschen/{email_oder_webhook}/{amtsgericht}/{grundbuchbezirk}/{blatt}")]
    async fn abo_loeschen(
        path: web::Path<(String, String, String, usize)>, 
        form: web::Json<AboNeuForm>, 
        req: HttpRequest,
    ) -> impl Responder {
        
        let (_token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => { return e; },
        };
        let (email_oder_webhook, amtsgericht, grundbuchbezirk, blatt) = &*path;
        
        let abo = crate::db::delete_abo(
            &email_oder_webhook, 
            &format!("{amtsgericht}/{grundbuchbezirk}/{blatt}"), 
            &benutzer.email, 
            form.tag.as_ref().map(|s| s.as_str())
        );

        let json = match abo {
            Err(e) => {
                serde_json::to_string_pretty(&AboNeuAnfrage::Err(AboNeuAnfrageErr {
                    code: 0,
                    text: format!("{e}"),
                })).unwrap_or_default()
            },
            Ok(()) => {
                serde_json::to_string_pretty(&AboNeuAnfrage::Ok(AboNeuAnfrageOk { }))
                .unwrap_or_default()
            }
        };
        
        HttpResponse::Ok()
        .content_type("application/json")
        .body(json)
    }
}