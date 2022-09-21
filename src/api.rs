use crate::models::BenutzerInfo;
use crate::AppState;
use actix_web::{HttpRequest, HttpResponse};
use std::collections::BTreeMap;

async fn get_benutzer_from_httpauth(
    req: &HttpRequest,
) -> Result<(String, BenutzerInfo), HttpResponse> {
    use self::upload::{UploadChangesetResponse, UploadChangesetResponseError};
    get_benutzer_from_httpauth_inner(req).await.map_err(|e| {
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

async fn get_benutzer_from_httpauth_inner(
    req: &HttpRequest,
) -> Result<(String, BenutzerInfo), String> {
    use actix_web::FromRequest;
    use actix_web_httpauth::extractors::bearer::BearerAuth;

    let token = match BearerAuth::extract(req).await {
        Ok(o) => o.token().to_string(),
        Err(e) => {
            let cookie = req.cookie("Authentication").ok_or(format!(
                "Konnte Authentifizierungs-Token nicht aus Bearer auslesen"
            ))?;
            cookie.value().to_string()
        }
    };

    let user = crate::db::get_user_from_token(&token)?;

    Ok((token, user))
}

pub(crate) async fn write_to_root_db(
    change: commit::DbChangeOp,
    app_state: &AppState,
) -> Result<(), String> {
    use crate::api::commit::{CommitResponse, CommitResponseOk};

    if !app_state.k8s_aktiv() {
        let result = crate::api::commit::db_change_inner(&change, app_state);

        crate::db::pull_db().await.map_err(|e| {
            format!(
                "Fehler beim Synchronisieren der Datenbanken (pull): {}: {}",
                e.code, e.text
            )
        })?;

        return result;
    };

    let k8s_sync_server_ip = crate::k8s::get_sync_server_ip()
        .await
        .map_err(|e| format!("Fehler beim Senden an /db: Kein Sync-Server: {e}"))?;

    let mut result = BTreeMap::new();

    let client = reqwest::Client::new();
    let res = client
        .post(&format!("http://{k8s_sync_server_ip}:8081/db"))
        .body(serde_json::to_string(&change).unwrap_or_default())
        .send()
        .await
        .map_err(|e| format!("Fehler beim Senden an /db: {e}"))?;

    let o = res.json::<CommitResponse>().await.map_err(|e| {
        format!("Konnte Änderung nicht an Sync-Server {k8s_sync_server_ip} senden: {e}")
    })?;

    match o {
        CommitResponse::StatusOk(CommitResponseOk {}) => {}
        CommitResponse::StatusError(e) => {
            result.insert(format!("{}", k8s_sync_server_ip), e);
        }
    }

    crate::db::pull_db().await.map_err(|e| {
        format!(
            "Fehler beim Synchronisieren der Datenbanken (pull): {}: {}",
            e.code, e.text
        )
    })?;

    if result.is_empty() {
        Ok(())
    } else {
        let error = result
            .iter()
            .map(|(k, v)| format!("{k}: {}: {}", v.code, v.text))
            .collect::<Vec<_>>()
            .join("\r\n");
        Err(format!("{error}"))
    }
}

/// HTML für `/` und `/api` Seite
pub mod index {
    use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    use crate::AppState;

    // Startseite
    #[get("/")]
    async fn status(_: HttpRequest) -> impl Responder {
        let css = include_str!("../web/style.css");
        let css = format!("<style type='text/css'>{css}</style>");
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(include_str!("../web/index.html").replace("<!-- CSS -->", &css))
    }

    // Zugriff anfragen
    #[get("/zugriff")]
    async fn zugriff(_: HttpRequest) -> impl Responder {
        let css = include_str!("../web/style.css");
        let css = format!("<style type='text/css'>{css}</style>");
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(include_str!("../web/zugriff.html").replace("<!-- CSS -->", &css))
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "action", content = "data")]
    pub enum ZugriffJsonPost {
        #[serde(rename = "get-amtsgerichte")]
        GetAmtsgerichte(ZugriffJsonGetAmtsgerichte),
        #[serde(rename = "get-bezirke")]
        GetBezirke(ZugriffJsonGetBezirke),
        #[serde(rename = "get-blaetter")]
        GetBlaetter(ZugriffJsonGetBlaetter),
        #[serde(rename = "anfrage")]
        Anfrage(ZugriffJsonAnfrage),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ZugriffJsonGetAmtsgerichte {
        land: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ZugriffJsonGetBezirke {
        land: String,
        amtsgericht: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ZugriffJsonGetBlaetter {
        land: String,
        amtsgericht: String,
        bezirk: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ZugriffJsonAnfrage {
        name: String,
        email: String,
        typ: String,
        grund: String,
        blaetter: Vec<ZugriffJsonAnfrageBlatt>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ZugriffJsonAnfrageBlatt {
        land: String,
        amtsgericht: String,
        grundbuchbezirk: String,
        blatt: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    enum ZugriffJsonResponse {
        #[serde(rename = "ok")]
        Ok(ZugriffJsonResponseOk),
        #[serde(rename = "error")]
        Error(ZugriffJsonResponseError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "action", content = "data")]
    enum ZugriffJsonResponseOk {
        #[serde(rename = "get-amtsgerichte")]
        GetAmtsgerichte(ZugriffJsonGetAmtsgerichteResponseOk),
        #[serde(rename = "get-bezirke")]
        GetBezirke(ZugriffJsonGetBezirkeResponseOk),
        #[serde(rename = "get-blaetter")]
        GetBlaetter(ZugriffJsonGetBlaetterResponseOk),
        #[serde(rename = "anfrage")]
        Anfrage(ZugriffJsonAnfrageResponseOk),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ZugriffJsonGetAmtsgerichteResponseOk {
        amtsgerichte: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ZugriffJsonGetBezirkeResponseOk {
        bezirke: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ZugriffJsonGetBlaetterResponseOk {
        blaetter: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ZugriffJsonAnfrageResponseOk {}

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ZugriffJsonResponseError {
        code: usize,
        text: String,
    }

    #[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
    pub enum ZugriffTyp {
        #[serde(rename = "gast")]
        Gast,
        #[serde(rename = "mitarbeiter")]
        Mitarbeiter,
        #[serde(rename = "bearbeiter")]
        Bearbeiter,
        #[serde(rename = "sonstige")]
        Sonstige,
    }

    impl ZugriffTyp {
        pub fn to_string(&self) -> String {
            serde_json::to_string(self).unwrap_or_default()
        }
        pub fn from_str(s: &str) -> Option<Self> {
            serde_json::from_str(s).ok()
        }
    }

    // Login-Seite
    #[post("/zugriff")]
    pub async fn zugriff_post(app_state: web::Data<AppState>, json: web::Json<ZugriffJsonPost>, _: HttpRequest) -> impl Responder {
        let response = zugriff_post_inner(
            &*app_state, 
            &*json
        ).await;

        HttpResponse::Ok()
            .content_type("application/json")
            .body(match response {
                Ok(o) => {
                    serde_json::to_string_pretty(&ZugriffJsonResponse::Ok(o)).unwrap_or_default()
                }
                Err(e) => serde_json::to_string_pretty(&ZugriffJsonResponse::Error(
                    ZugriffJsonResponseError {
                        code: 500,
                        text: e.clone(),
                    },
                ))
                .unwrap_or_default(),
            })
    }

    async fn zugriff_post_inner(app_state: &AppState, json: &ZugriffJsonPost) -> Result<ZugriffJsonResponseOk, String> {
        use self::ZugriffJsonPost::*;

        match json {
            GetAmtsgerichte(ga) => {
                let amtsgerichte = crate::db::get_amtsgerichte_for_bundesland(&ga.land)?;
                Ok(ZugriffJsonResponseOk::GetAmtsgerichte(
                    ZugriffJsonGetAmtsgerichteResponseOk { amtsgerichte },
                ))
            }
            GetBezirke(gb) => {
                let bezirke = crate::db::get_bezirke_for_amtsgericht(&gb.amtsgericht)?;
                Ok(ZugriffJsonResponseOk::GetBezirke(
                    ZugriffJsonGetBezirkeResponseOk { bezirke },
                ))
            }
            GetBlaetter(gb) => {
                let blaetter =
                    crate::db::get_blaetter_for_bezirk(&gb.land, &gb.amtsgericht, &gb.bezirk)?;
                Ok(ZugriffJsonResponseOk::GetBlaetter(
                    ZugriffJsonGetBlaetterResponseOk { blaetter },
                ))
            }
            Anfrage(a) => {
                use super::commit::DbChangeOp;

                if a.name.is_empty() {
                    return Err(format!("Kein Name angegeben"));
                }
                if a.email.is_empty() {
                    return Err(format!("Keine E-Mail angegeben"));
                }
                let typ = match a.typ.as_str() {
                    "GAST" => ZugriffTyp::Gast,
                    "M-OD" => ZugriffTyp::Mitarbeiter,
                    "B-OD" => ZugriffTyp::Bearbeiter,
                    "SONSTIGE" => {
                        if a.grund.trim().is_empty() {
                            return Err(
                                "Kein Berechtigungsgrund angegeben für Typ \"Sonstiger Grund\""
                                    .to_string(),
                            );
                        }
                        ZugriffTyp::Sonstige
                    }
                    _ => {
                        return Err(format!("Ungültiger \"typ\" angegeben"));
                    }
                };

                let now = chrono::Utc::now().to_rfc3339();

                for blatt in a.blaetter.iter() {
                    let zugriff_id = uuid::Uuid::new_v4();
                    let zugriff_id = format!("{zugriff_id}");

                    crate::api::write_to_root_db(
                        DbChangeOp::CreateZugriff {
                            id: zugriff_id,
                            name: a.name.trim().to_string(),
                            email: a.email.trim().to_string(),
                            typ: typ,
                            grund: a.grund.trim().to_string(),
                            datum: now.clone(),
                            land: blatt.land.trim().to_string(),
                            amtsgericht: blatt.amtsgericht.trim().to_string(),
                            bezirk: blatt.grundbuchbezirk.trim().to_string(),
                            blatt: blatt.blatt.trim().to_string(),
                        },
                        app_state,
                    )
                    .await?;
                }

                Ok(ZugriffJsonResponseOk::Anfrage(
                    ZugriffJsonAnfrageResponseOk {},
                ))
            }
        }
    }

    #[get("/konto.js")]
    async fn konto_js(_: HttpRequest) -> impl Responder {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("web")
            .join("konto.js");

        let s = std::fs::read_to_string(path).unwrap_or_default();

        HttpResponse::Ok().content_type("text/javascript").body(s)
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
        let body = format!(
            "
            <!DOCTYPE html>
            <html>
                <head><style>{css}</style></head>
                <body>
                <nav>
                    <ul>
                        <li>
                            <a href='/' class='block-btn'><span>Startseite</span></a>
                            <a href='/konto' class='block-btn'><span>Mein Konto</span></a>
                        </li>
                    </ul>
                </nav>
                <div class='readme'>
                {html}
                </div>
                </body>
            </html>
        "
        );

        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(body)
    }
}

/// Login-API
pub mod login {

    use crate::{AppState, MountPoint};
    use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
    use chrono::{DateTime, Utc};
    use serde_derive::{Deserialize, Serialize};

    // Login-Seite
    #[get("/login")]
    async fn login_get(_: HttpRequest) -> impl Responder {
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
        form: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    pub enum LoginResponse {
        #[serde(rename = "ok")]
        Ok(LoginResponseOk),
        #[serde(rename = "error")]
        Error(LoginResponseError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct LoginResponseOk {
        pub token: String,
        pub valid_until: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct LoginResponseError {
        pub code: usize,
        pub text: String,
    }

    // Login-Seite
    #[post("/login")]
    async fn login_post(
        app_state: web::Data<AppState>,
        form: web::Form<LoginForm>,
        _: HttpRequest,
    ) -> impl Responder {
        let response = login_json(&form.email, &form.passwort, &*app_state).await;
        HttpResponse::Ok()
            .content_type("application/json; charset=utf-8")
            .body(serde_json::to_string_pretty(&response).unwrap_or_default())
    }

    pub async fn login_json(email: &str, passwort: &str, app_state: &AppState) -> LoginResponse {
        use crate::api::commit::DbChangeOp;

        match crate::db::check_password(MountPoint::Local, &email, &passwort) {
            // Benutzer + Token existiert
            Ok((_info, token, valid_until)) => {
                LoginResponse::Ok(LoginResponseOk { token, valid_until })
            }
            // Benutzer existiert nicht
            Err(Some(e)) => LoginResponse::Error(LoginResponseError {
                code: 0,
                text: e.clone(),
            }),
            // Benutzer existiert, aber noch kein Token
            Err(None) => {
                let (token, gueltig_bis) = crate::db::generate_token();

                match crate::api::write_to_root_db(
                    DbChangeOp::BenutzerSessionNeu {
                        email: email.to_string(),
                        token: token.clone(),
                        gueltig_bis: gueltig_bis.clone(),
                    },
                    app_state,
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        return LoginResponse::Error(LoginResponseError {
                            code: 500,
                            text: e.clone(),
                        });
                    }
                }

                match crate::db::check_password(MountPoint::Local, &email, &passwort) {
                    Ok((_info, token, valid_until)) => {
                        LoginResponse::Ok(LoginResponseOk { token, valid_until })
                    }
                    Err(e) => LoginResponse::Error(LoginResponseError {
                        code: 0,
                        text: e.unwrap_or_default().clone(),
                    }),
                }
            }
        }
    }
}

/// API für `/konto` Anfragen: Gibt HTML-Übersicht für Benutzer / Abo-Verwaltung
pub mod konto {
    use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};

    use crate::{db::{KontoData, GpgKeyPair, KontoDataResult}, BezirkeLoeschenArgs, BenutzerNeuArgsJson, AppState, BenutzerLoeschenArgs, BezirkeNeuArgs};

    // Konto-Seite
    #[get("/konto")]
    async fn konto_get(req: HttpRequest) -> impl Responder {

        let (token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(_) => {
                return HttpResponse::Found()
                    .append_header(("Location", "/login"))
                    .finish();
            }
        };

        let konto_data = match crate::db::get_konto_data(&benutzer) {
            Ok(KontoDataResult::Aktiviert(a)) => a,
            Ok(KontoDataResult::KeinPasswort) => {
                let html = include_str!("../web/konto-login.html")
                .replace(
                    "<!-- CSS -->",
                    &format!("<style>{}</style>", include_str!("../web/style.css")),
                );
                return HttpResponse::Ok()
                    .content_type("text/html; charset=utf-8")
                    .body(html);
            },
            Err(e) => {
                KontoData::default()
            }
        };

        let konto_data_json = serde_json::to_string(&konto_data).unwrap_or_default();

        let html = include_str!("../web/konto.html")
            .replace(
                "<!-- CSS -->",
                &format!("<style>{}</style>", include_str!("../web/style.css")),
            )
            .replace(
                "data-konto-daten=\"{}\"",
                &format!("data-konto-daten=\'{}\'", konto_data_json),
            )
            .replace(
                "data-token-id=\"\"",
                &format!("data-token-id=\"{}\"", token),
            );

        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html)
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct KontoJsonPost {
        auth: String,
        aktion: String,
        daten: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    enum KontoJsonPostResponse {
        #[serde(rename = "ok")]
        Ok(KontoData),
        #[serde(rename = "error")]
        Error(KontoJsonPostResponseError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct KontoJsonPostResponseError {
        code: usize,
        text: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    enum KontoGeneriereSchluesselPostResponse {
        #[serde(rename = "ok")]
        Ok(GpgKeyPair),
        #[serde(rename = "error")]
        Error(KontoGeneriereSchluesselPostResponseError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct KontoGeneriereSchluesselPostResponseError {
        code: usize,
        text: String,
    }


    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct KontoGeneriereSchluessel {
        auth: String,
        email: String,
        name: String,    
    }

    #[post("/konto-generiere-schluessel")]
    async fn konto_generiere_schluessel(_: HttpRequest, state: web::Data<AppState>, data: web::Json<KontoGeneriereSchluessel>) -> impl Responder {
        let result =  match konto_generiere_schluessel_inner(&*state, &*data).await {
            Ok(o) => KontoGeneriereSchluesselPostResponse::Ok(o),
            Err(e) => KontoGeneriereSchluesselPostResponse::Error(e),
        };

        HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&result).unwrap_or_default())
    }
    
    async fn konto_generiere_schluessel_inner(
        app_state: &AppState, 
        data: &KontoGeneriereSchluessel
    ) -> Result<GpgKeyPair, KontoGeneriereSchluesselPostResponseError> {
    
        let benutzer = crate::db::get_user_from_token(&data.auth)
        .map_err(|e| KontoGeneriereSchluesselPostResponseError {
            code: 500,
            text: e,
        })?;

        if benutzer.rechte != "admin" {
            return Err(KontoGeneriereSchluesselPostResponseError {
                code: 500,
                text: "Cannot generate public key pair: Invalid authentication.".to_string(),
            });
        }

        let pair = crate::db::create_gpg_key(&data.name, &data.email)
            .map_err(|e| KontoGeneriereSchluesselPostResponseError {
                code: 500,
                text: format!("Cannot generate public key pair: Unknown error."),
            })?;

        Ok(pair)
    }

    #[post("/konto")]
    async fn konto_post(_: HttpRequest, state: web::Data<AppState>, data: web::Json<KontoJsonPost>) -> impl Responder {
        let result =  match konto_post_inner(&*state, &*data).await {
            Ok(o) => KontoJsonPostResponse::Ok(o),
            Err(e) => KontoJsonPostResponse::Error(e),
        };

        HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&result).unwrap_or_default())
    }

    async fn konto_post_inner(app_state: &AppState, data: &KontoJsonPost) -> Result<KontoData, KontoJsonPostResponseError> {

        use crate::api::commit::DbChangeOp;
        use crate::BezirkNeuArgs;

        let benutzer = crate::db::get_user_from_token(&data.auth)
        .map_err(|e| KontoJsonPostResponseError {
            code: 500,
            text: e,
        })?;

        match (benutzer.rechte.as_str(), data.aktion.as_str()) {
            ("admin", "benutzer-neu") => {
                let name = data.daten.get(0)
                .ok_or(KontoJsonPostResponseError {
                    code: 500,
                    text: "Benutzer neu anlegen: kein Name in Daten vorhanden".to_string(),
                })?;
                let email = data.daten.get(1)
                .ok_or(KontoJsonPostResponseError {
                    code: 500,
                    text: "Benutzer neu anlegen: keine E-Mail in Daten vorhanden".to_string(),
                })?;
                let passwort = data.daten.get(2)
                .ok_or(KontoJsonPostResponseError {
                    code: 500,
                    text: "Benutzer neu anlegen: kein Passwort in Daten vorhanden".to_string(),
                })?;
                let rechte = "gast";
                crate::api::write_to_root_db(DbChangeOp::BenutzerNeu(BenutzerNeuArgsJson {
                    name: name.clone(),
                    email: email.clone(),
                    passwort: passwort.clone(),
                    rechte: rechte.to_string(),
                    schluessel: None,
                }), &app_state)
                    .await
                    .map_err(|e| KontoJsonPostResponseError {
                        code: 500,
                        text: e,
                    })?;
            },
            ("admin", "benutzer-loeschen") => {
                let email = data.daten.get(0)
                .ok_or(KontoJsonPostResponseError {
                    code: 500,
                    text: "Benutzer löschen: keine E-Mail in Daten vorhanden".to_string(),
                })?;
                crate::api::write_to_root_db(DbChangeOp::BenutzerLoeschen(BenutzerLoeschenArgs {
                    email: email.clone(),
                }), &app_state)
                    .await
                    .map_err(|e| KontoJsonPostResponseError {
                        code: 500,
                        text: e,
                    })?;
            },
            ("admin", "benutzer-bearbeite-kontotyp") => {
                let neuer_kontotyp = data.daten.get(0)
                .ok_or(KontoJsonPostResponseError {
                    code: 500,
                    text: "Benutzer neu anlegen: kein Name in Daten vorhanden".to_string(),
                })?;

                let kontotypen = ["admin", "gast", "bearbeiter"];
                if !kontotypen.iter().any(|k| *k == neuer_kontotyp) {
                    return Err(KontoJsonPostResponseError {
                        code: 500,
                        text: "Benutzer-Kontotyp bearbeiten: Ungültiger Kontotyp".to_string(),
                    });
                }

                let emails = data.daten.iter().skip(1).cloned().collect();

                crate::api::write_to_root_db(DbChangeOp::BenutzerAendernRechte {
                    neue_rechte: neuer_kontotyp.to_string(),
                    ids: emails,
                }, &app_state)
                    .await
                    .map_err(|e| KontoJsonPostResponseError {
                        code: 500,
                        text: e,
                    })?;
            },
            ("admin", "benutzer-bearbeite-pubkey") => {

                let email = data.daten.get(0)
                .ok_or(KontoJsonPostResponseError {
                    code: 500,
                    text: "Benutzer PubKey ändern: keine E-Mail ID".to_string(),
                })?;

                let pubkey = data.daten.get(1)
                .ok_or(KontoJsonPostResponseError {
                    code: 500,
                    text: "Benutzer PubKey ändern: keine PubKey".to_string(),
                })?;

                crate::api::write_to_root_db(DbChangeOp::BenutzerAendernPubkey { 
                    id: email.clone(), 
                    neuer_pubkey: pubkey.clone(),
                }, &app_state)
                    .await
                    .map_err(|e| KontoJsonPostResponseError {
                        code: 500,
                        text: e,
                    })?;
            },
            ("admin", "bezirk-neu") => {
                let mut bezirke = Vec::new();
                // daten =  CSV-Datei als Zellen (Land, Amtsgericht, Bezirk)
                for (i, triple) in data.daten.chunks(3).enumerate() {
                    let land = triple.get(0)
                    .ok_or(KontoJsonPostResponseError {
                        code: 500,
                        text: format!("Bezirke neu anlegen: Fehler in Zeile {i}: Bundesland fehlt"),
                    })?;
                    let amtsgericht = triple.get(1)
                    .ok_or(KontoJsonPostResponseError {
                        code: 500,
                        text: format!("Bezirke neu anlegen: Fehler in Zeile {i}: Amtsgericht fehlt"),
                    })?;
                    let bezirk = triple.get(2)
                    .ok_or(KontoJsonPostResponseError {
                        code: 500,
                        text: format!("Bezirke neu anlegen: Fehler in Zeile {i}: Grundbuchbezirk fehlt"),
                    })?;
                    bezirke.push(BezirkNeuArgs {
                        land: land.clone(), 
                        amtsgericht: amtsgericht.clone(), 
                        bezirk: bezirk.clone()
                    });
                }

                crate::api::write_to_root_db(DbChangeOp::BezirkeNeu(BezirkeNeuArgs {
                    bezirke: bezirke,
                }), &app_state)
                    .await
                    .map_err(|e| KontoJsonPostResponseError {
                        code: 500,
                        text: e,
                    })?;
            },
            ("admin", "bezirk-loeschen") => {
                crate::api::write_to_root_db(DbChangeOp::BezirkeLoeschen(BezirkeLoeschenArgs {
                    ids: data.daten.clone(),
                }), &app_state)
                    .await
                    .map_err(|e| KontoJsonPostResponseError {
                        code: 500,
                        text: e,
                    })?;
            },
            ("admin", "zugriff-genehmigen") => {
                crate::api::write_to_root_db(DbChangeOp::ZugriffGenehmigen {
                    ids: data.daten.clone(),
                    email: benutzer.email.clone(),
                    datum: chrono::Utc::now().to_rfc3339(),
                }, &app_state)
                    .await
                    .map_err(|e| KontoJsonPostResponseError {
                        code: 500,
                        text: e,
                    })?;

                /*
                    let smtp_config = crate::db::get_email_config()?;
                    crate::email::send_zugriff_gewaehrt_email(
                        &smtp_config
                    )?;
                */
            },
            ("admin", "zugriff-ablehnen") => {
                crate::api::write_to_root_db(DbChangeOp::ZugriffAblehnen {
                    ids: data.daten.clone(),
                    email: benutzer.email.clone(),
                    datum: chrono::Utc::now().to_rfc3339(),
                }, &app_state)
                    .await
                    .map_err(|e| KontoJsonPostResponseError {
                        code: 500,
                        text: e,
                    })?;

                // sende_email(&benutzer.email, zugriff_abgelehnt)
            },
            ("admin", "konfiguration-bearbeiten") => {
                for pair in data.daten.chunks(2) {
                    crate::api::write_to_root_db(DbChangeOp::BearbeiteEinstellung {
                        id: pair[0].to_string(),
                        neuer_wert: pair[1].to_string(),
                    }, &app_state)
                        .await
                        .map_err(|e| KontoJsonPostResponseError {
                            code: 500,
                            text: e,
                        })?;
                }
            },
            _ => {
                return Err(KontoJsonPostResponseError {
                    code: 500,
                    text: "Benutzer ist zum Ausführen dieser Aktion nicht authorisiert".to_string(),
                })
            }
        }

        let konto_data = 
            crate::db::get_konto_data(&benutzer)
            .unwrap_or(KontoDataResult::KeinPasswort);

        let konto_data = match konto_data {
            KontoDataResult::Aktiviert(a) => a,
            KontoDataResult::KeinPasswort => KontoData::default(),
        };

        Ok(konto_data)
    }
}

/// Wenn der Server im "Synchronisierungsmodus" gestartet wird,
/// öffnet er einen Port auf :8081 (welcher nicht über den LoadBalancer)
/// öffentlich pingbar ist. Der "Synchronisierungs-Server" überwacht alle
/// Dateien im PersistentVolume, und pingt alle anderen Server im Cluster
/// an, wenn sich Dateien verändern.
///
/// Die angepingten Server wiederum kopieren sich den neuen Stand der Dateien
/// wieder in den Pod-lokalen Speicher. So findet eine "asynchrone" Synchronisierung
/// statt, bei der immer mindestens zwei Kopien des gesamten Dateibestands existieren.
pub mod commit {

    use super::{
        pull::PullResponse,
        upload::{commit_changes, sync_changes_to_disk, verify_signature, UploadChangeset},
    };
    use crate::models::{get_data_dir, get_db_path, MountPoint};
    use crate::{
        AboLoeschenArgs, AboNeuArgs, AppState, BenutzerLoeschenArgs, BenutzerNeuArgsJson,
        BezirkLoeschenArgs, BezirkNeuArgs, BenutzerAendernArgs, BezirkeNeuArgs,
        BezirkeLoeschenArgs,
    };
    use crate::api::index::ZugriffTyp;
    use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
    use chrono::{DateTime, Utc};
    use serde_derive::{Deserialize, Serialize};
    use std::path::Path;

    #[post("/ping")]
    async fn ping(_: HttpRequest) -> impl Responder {
        HttpResponse::Ok().body("")
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    pub enum CommitResponse {
        #[serde(rename = "ok")]
        StatusOk(CommitResponseOk),
        #[serde(rename = "error")]
        StatusError(CommitResponseError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CommitResponseOk {}

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CommitResponseError {
        pub code: usize,
        pub text: String,
    }

    #[post("/commit")]
    async fn commit(
        upload_changeset: web::Json<UploadChangeset>,
        app_state: web::Data<AppState>,
        req: HttpRequest,
    ) -> impl Responder {
        match commit_internal(&upload_changeset, &app_state, &req).await {
            Ok(o) => o,
            Err(e) => e,
        }
    }

    async fn commit_internal(
        upload_changeset: &UploadChangeset,
        app_state: &AppState,
        req: &HttpRequest,
    ) -> Result<HttpResponse, HttpResponse> {
        let upload_changeset = &*upload_changeset;
        let (token, benutzer) = super::get_benutzer_from_httpauth(&req).await?;

        let response_err = |code: usize, text: String| {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string_pretty(&CommitResponse::StatusError(CommitResponseError {
                    code: code,
                    text: text,
                }))
                .unwrap_or_default(),
            )
        };

        let response_ok = || {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string(&CommitResponse::StatusOk(CommitResponseOk {}))
                    .unwrap_or_default(),
            )
        };

        verify_signature(&benutzer.email, &upload_changeset).map_err(|e| {
            response_err(
                501,
                format!("Fehler bei Überprüfung der digitalen Signatur:\r\n{e}"),
            )
        })?;

        if app_state.k8s_aktiv() && app_state.sync_server() {
            let remote_path = Path::new(&get_data_dir(MountPoint::Remote)).to_path_buf();
            sync_changes_to_disk(&upload_changeset, &remote_path)?;
            commit_changes(&app_state, &remote_path, &benutzer, &upload_changeset)
                .await
                .map_err(|e| {
                    response_err(501, format!("Konnte Änderung nicht speichern:\r\n{e}"))
                })?;

            let k8s_peers = crate::k8s::k8s_get_peer_ips().await
            .map_err(|_| response_err(500, "Kubernetes aktiv, konnte aber Pods nicht lesen (keine ClusterRole-Berechtigung?)".to_string()))?;

            for peer in k8s_peers.iter() {
                if !peer.name.starts_with("dgb-server") {
                    continue;
                }
                let client = reqwest::Client::new();
                let res = client
                    .post(&format!("http://{}:8081/pull", peer.ip))
                    .body("")
                    .send()
                    .await;

                let json = match res {
                    Ok(o) => o.json::<PullResponse>().await,
                    Err(e) => {
                        log::error!(
                            "Pod {}:{} konnte nicht synchronisiert werden: {e}",
                            peer.namespace,
                            peer.name
                        );
                        continue;
                    }
                };

                match json {
                    Ok(PullResponse::StatusOk(_)) => {}
                    Ok(PullResponse::StatusError(e)) => {
                        log::error!(
                            "Pod {}:{} konnte nicht synchronisiert werden: {}: {}",
                            peer.namespace,
                            peer.name,
                            e.code,
                            e.text
                        );
                        continue;
                    }
                    Err(e) => {
                        log::error!(
                            "Pod {}:{} konnte nicht synchronisiert werden: {e}",
                            peer.namespace,
                            peer.name
                        );
                        continue;
                    }
                }
            }
        } else {
            let local_path = Path::new(&get_data_dir(MountPoint::Local)).to_path_buf();
            sync_changes_to_disk(&upload_changeset, &local_path)?;
            let _ = commit_changes(&app_state, &local_path, &benutzer, &upload_changeset).await;
        }

        Ok(response_ok())
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(crate) enum DbChangeOp {
        BenutzerNeu(BenutzerNeuArgsJson),
        BenutzerAendernRechte {
            ids: Vec<String>, 
            neue_rechte: String,
        },
        BenutzerAendernPubkey {
            id: String, 
            neuer_pubkey: String,
        },
        BenutzerLoeschen(BenutzerLoeschenArgs),
        BezirkNeu(BezirkNeuArgs),
        // Mehrere Bezirke gleichzeitig mit Datenbank abgleichen
        BezirkeNeu(BezirkeNeuArgs),
        BezirkLoeschen(BezirkLoeschenArgs),
        // Mehrere Bezirke anhand ID löschen
        BezirkeLoeschen(BezirkeLoeschenArgs),
        AboNeu(AboNeuArgs),
        AboLoeschen(AboLoeschenArgs),
        CreateZugriff {
            id: String,
            name: String,
            email: String,
            typ: ZugriffTyp,
            grund: String,
            datum: String,
            land: String,
            amtsgericht: String,
            bezirk: String,
            blatt: String,
        },
        ZugriffGenehmigen {
            ids: Vec<String>,
            email: String,
            datum: String,
        },
        ZugriffAblehnen {
            ids: Vec<String>,
            email: String,
            datum: String,
        },
        BenutzerSessionNeu {
            email: String,
            token: String,
            gueltig_bis: DateTime<Utc>,
        },
        BearbeiteEinstellung {
            id: String, 
            neuer_wert: String,
        }
    }

    #[post("/get-db")]
    async fn get_db(app_state: web::Data<AppState>, _: HttpRequest) -> HttpResponse {
        use lz4_flex::compress_prepend_size;

        if app_state.k8s_aktiv() && app_state.sync_server() {
            let db_bytes = match std::fs::read(get_db_path(MountPoint::Remote)) {
                Ok(o) => o,
                Err(_) => return HttpResponse::NotFound().finish(),
            };
            let compressed = compress_prepend_size(&db_bytes);
            HttpResponse::Ok().body(compressed)
        } else {
            let db_bytes = match std::fs::read(get_db_path(MountPoint::Local)) {
                Ok(o) => o,
                Err(_) => return HttpResponse::NotFound().finish(),
            };
            let compressed = compress_prepend_size(&db_bytes);
            HttpResponse::Ok().body(compressed)
        }
    }

    #[post("/db")]
    async fn db(
        upload_changeset: web::Json<DbChangeOp>,
        app_state: web::Data<AppState>,
        _: HttpRequest,
    ) -> impl Responder {
        match db_change_internal(&upload_changeset, &app_state) {
            Ok(o) => o,
            Err(e) => e,
        }
    }

    pub(crate) fn db_change_inner(
        change_op: &DbChangeOp,
        app_state: &AppState,
    ) -> Result<(), String> {
        let mount_point_write = if app_state.k8s_aktiv() && app_state.sync_server() {
            MountPoint::Remote
        } else {
            MountPoint::Local
        };

        match change_op {
            DbChangeOp::BearbeiteEinstellung {
                id, neuer_wert,
            } => crate::db::bearbeite_einstellung(
                mount_point_write, 
                id, 
                neuer_wert
            ),
            DbChangeOp::BenutzerNeu(un) => crate::db::create_user(
                mount_point_write,
                &un.name,
                &un.email,
                &un.passwort,
                &un.rechte,
                un.schluessel.clone(),
            ),
            DbChangeOp::BenutzerAendernRechte { 
                ids,
                neue_rechte,
            } => crate::db::bearbeite_benutzer_rechte(
                mount_point_write,
                ids,
                neue_rechte
            ),
            DbChangeOp::BenutzerAendernPubkey {
                id, 
                neuer_pubkey,
            } => crate::db::bearbeite_benutzer_pubkey(
                mount_point_write, 
                id, 
                neuer_pubkey,
            ),
            DbChangeOp::BezirkeLoeschen(b) => crate::db::bezirke_loeschen(
                mount_point_write,
                b.ids.as_ref(),
            ),
            DbChangeOp::BenutzerLoeschen(ul) => {
                crate::db::delete_user(mount_point_write, &ul.email)
            }
            DbChangeOp::BezirkNeu(bn) => crate::db::create_gemarkung(
                mount_point_write,
                &bn.land,
                &bn.amtsgericht,
                &bn.bezirk,
            ),
            DbChangeOp::BezirkeNeu(b) => crate::db::bezirke_einfuegen(
                mount_point_write,
                &b.bezirke,
            ),
            DbChangeOp::BezirkLoeschen(bl) => crate::db::delete_gemarkung(
                mount_point_write,
                &bl.land,
                &bl.amtsgericht,
                &bl.bezirk,
            ),
            DbChangeOp::AboNeu(an) => crate::db::create_abo(
                mount_point_write,
                &an.typ,
                &an.blatt,
                &an.text,
                an.aktenzeichen.as_ref().map(|s| s.as_str()),
            ),
            DbChangeOp::AboLoeschen(al) => crate::db::delete_abo(
                mount_point_write,
                &al.typ,
                &al.blatt,
                &al.text,
                al.aktenzeichen.as_ref().map(|s| s.as_str()),
            ),
            DbChangeOp::CreateZugriff {
                id,
                name,
                email,
                typ,
                grund,
                datum,
                land,
                amtsgericht,
                bezirk,
                blatt,
            } => crate::db::create_zugriff(
                mount_point_write,
                id,
                name,
                email,
                *typ,
                grund,
                datum,
                land,
                amtsgericht,
                bezirk,
                blatt,
            ),
            DbChangeOp::ZugriffGenehmigen { ids, email, datum } => {
                crate::db::zugriff_genehmigen(mount_point_write, ids, email, datum)
            },
            DbChangeOp::ZugriffAblehnen { ids, email, datum } => {
                crate::db::zugriff_ablehnen(mount_point_write, ids, email, datum)
            }
            DbChangeOp::BenutzerSessionNeu {
                email,
                token,
                gueltig_bis,
            } => {
                crate::db::insert_token_into_sessions(mount_point_write, email, token, gueltig_bis)
            }
        }
    }

    fn db_change_internal(
        change_op: &DbChangeOp,
        app_state: &AppState,
    ) -> Result<HttpResponse, HttpResponse> {
        let response_err = |code: usize, text: String| {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string_pretty(&CommitResponse::StatusError(CommitResponseError {
                    code: code,
                    text: text,
                }))
                .unwrap_or_default(),
            )
        };

        let response_ok = || {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string(&CommitResponse::StatusOk(CommitResponseOk {}))
                    .unwrap_or_default(),
            )
        };

        match db_change_inner(change_op, app_state) {
            Ok(()) => Ok(response_ok()),
            Err(e) => {
                println!("error in db: {e}");
                Err(response_err(500, e))
            },
        }
    }
}

/// Um die Server zu synchronisieren, läuft intern ein zweiter Server auf Port 8081,
/// der nur im K8s-Cluster intern anpingbar ist. Wenn der Server über /pull oder /pull-db
/// angepingt wird, wird die Pod-lokale Datenbank mit dem PersistentVolume synchronisiert
/// (meist nach Insert / Delete Abfragen).
///
/// Insgesamt verhindert dieses Vorgehen, dass es zu Verzögerungen / Ausfällen bei Arbeiten
/// am PersistentVolume kommt.
pub mod pull {

    use crate::models::{get_data_dir, get_db_path, MountPoint};
    use crate::AppState;
    use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    use std::path::Path;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    pub enum PullResponse {
        #[serde(rename = "ok")]
        StatusOk(PullResponseOk),
        #[serde(rename = "error")]
        StatusError(PullResponseError),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PullResponseOk {}

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PullResponseError {
        pub code: usize,
        pub text: String,
    }

    #[post("/pull")]
    pub async fn pull(_: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
        match pull_internal(&app_state).await {
            Ok(o) => o,
            Err(e) => e,
        }
    }

    async fn pull_internal(app_state: &AppState) -> Result<HttpResponse, HttpResponse> {
        use git2::Repository;

        let response_ok = || {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string(&PullResponse::StatusOk(PullResponseOk {}))
                    .unwrap_or_default(),
            )
        };

        let response_err = |code: usize, text: String| {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string(&PullResponse::StatusError(PullResponseError { code, text }))
                    .unwrap_or_default(),
            )
        };

        if !app_state.k8s_aktiv() || app_state.sync_server() {
            return Ok(response_ok());
        }

        let local_path = Path::new(&get_data_dir(MountPoint::Local)).to_path_buf();
        if !local_path.exists() {
            let _ = std::fs::create_dir(local_path.clone());
        }

        let repo = match Repository::open(&local_path) {
            Ok(o) => o,
            Err(_) => {
                Repository::init(&local_path).map_err(|e| response_err(501, format!("{e}")))?
            }
        };

        let sync_server_ip = crate::k8s::get_sync_server_ip()
            .await
            .map_err(|e| response_err(501, format!("Konnte Sync-Server nicht finden: {e}")))?;

        let data_remote = format!("git://{sync_server_ip}:9418/");
        repo.remote_add_fetch("origin", &data_remote)
            .map_err(|e| response_err(501, format!("git_clone({data_remote}): {e}")))?;

        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| response_err(501, format!("git_clone({data_remote}): {e}")))?;

        remote
            .fetch(&["main"], None, None)
            .map_err(|e| response_err(501, format!("git_clone({data_remote}): {e}")))?;

        Ok(response_ok())
    }

    #[post("/pull-db")]
    pub async fn pull_db(_: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
        let result = pull_db_internal(&app_state).await;
        match result {
            Ok(o) => o,
            Err(e) => e,
        }
    }

    async fn pull_db_internal(app_state: &AppState) -> Result<HttpResponse, HttpResponse> {
        let response_ok = || {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string(&PullResponse::StatusOk(PullResponseOk {}))
                    .unwrap_or_default(),
            )
        };

        let response_err = |code: usize, text: String| {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string(&PullResponse::StatusError(PullResponseError { code, text }))
                    .unwrap_or_default(),
            )
        };

        if !app_state.k8s_aktiv() || app_state.sync_server() {
            return Ok(response_ok());
        }

        let remote_db_bytes = crate::db::get_db_bytes()
            .await
            .map_err(|e| response_err(500, format!("get_db_bytes: {e}")))?;

        let local_path = Path::new(&get_db_path(MountPoint::Local)).to_path_buf();
        if let Some(parent) = local_path.parent() {
            let _ = std::fs::create_dir(parent);
        }

        let _ = std::fs::write(&local_path, &remote_db_bytes).map_err(|e| {
            response_err(
                500,
                format!("Remote: Fehler beim Kopieren der Benutzerdatenbank vom PV zum Pod: {e}"),
            )
        });

        Ok(response_ok())
    }
}

/// API für `/upload` Anfragen
pub mod upload {

    use super::commit::CommitResponse;
    use crate::{
        db::GemarkungsBezirke,
        models::{get_data_dir, BenutzerInfo, MountPoint, PdfFile},
        AppState,
    };
    use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

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
    pub struct UploadChangesetResponseOk {}

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct UploadChangesetResponseError {
        pub code: usize,
        pub text: String,
    }

    #[post("/upload")]
    async fn upload(
        upload_changeset: web::Json<UploadChangeset>,
        app_state: web::Data<AppState>,
        req: HttpRequest,
    ) -> impl Responder {
        match upload_internal(&upload_changeset, &app_state, &req).await {
            Ok(o) => o,
            Err(e) => e,
        }
    }

    async fn upload_internal(
        upload_changeset: &UploadChangeset,
        app_state: &AppState,
        req: &HttpRequest,
    ) -> Result<HttpResponse, HttpResponse> {
        let upload_changeset = &*upload_changeset;
        let (token, benutzer) = super::get_benutzer_from_httpauth(&req).await?;

        let response_err = |code: usize, text: String| {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                    UploadChangesetResponseError {
                        code: code,
                        text: text,
                    },
                ))
                .unwrap_or_default(),
            )
        };

        let response_ok = || {
            HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string(&UploadChangesetResponse::StatusOk(
                    UploadChangesetResponseOk {},
                ))
                .unwrap_or_default(),
            )
        };

        verify_signature(&benutzer.email, &upload_changeset).map_err(|e| {
            response_err(
                501,
                format!("Fehler bei Überprüfung der digitalen Signatur:\r\n{e}"),
            )
        })?;

        if app_state.k8s_aktiv() && !app_state.sync_server() {
            let k8s_sync_server_ip = crate::k8s::get_sync_server_ip().await
            .map_err(|e| response_err(500, "Kubernetes aktiv, konnte Pods aber nicht lesen (keine ClusterRole-Berechtigung?)".to_string()))?;

            let client = reqwest::Client::new();
            let res = client
                .post(&format!("http://{k8s_sync_server_ip}:8081/commit"))
                .body(serde_json::to_string(&upload_changeset).unwrap_or_default())
                .bearer_auth(token.clone())
                .send()
                .await;

            let json = res.map_err(|e| response_err(500, format!("{e}")))?;

            if let Some(cr) = json.json::<CommitResponse>().await.ok() {
                match cr {
                    CommitResponse::StatusOk(_) => return Ok(response_ok()),
                    CommitResponse::StatusError(e) => {
                        return Err(response_err(e.code, e.text));
                    }
                }
            }

            return Err(response_err(
                500,
                "Konnte Änderung nicht speichern: kein Synchronisationsserver aktiv.".to_string(),
            ));
        } else {
            let local_path = Path::new(&get_data_dir(MountPoint::Local)).to_path_buf();
            sync_changes_to_disk(&upload_changeset, &local_path)?;
            commit_changes(&app_state, &local_path, &benutzer, &upload_changeset)
                .await
                .map_err(|e| response_err(500, format!("Konnte Änderung nicht speichern: {e}")))?;
            Ok(response_ok())
        }
    }

    pub fn verify_signature(email: &str, changeset: &UploadChangeset) -> Result<bool, String> {
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

        let p = &P::new();
        let cert = crate::db::get_key_for_fingerprint(&changeset.fingerprint, email)?;
        let mut plaintext = Vec::new();
        let _ = crate::pgp::verify(p, &mut plaintext, msg.as_bytes(), &cert)
            .map_err(|e| format!("{e}"))?;

        Ok(true)
    }

    fn commit_header_with_signature(
        commit_titel: &str,
        commit_beschreibung: &[String],
        fingerprint: &str,
        signatur: &PgpSignatur,
    ) -> String {
        let mut no_sig = String::new();

        no_sig.push_str(commit_titel);
        no_sig.push_str("\r\n\r\n");

        if !commit_beschreibung.is_empty() {
            no_sig.push_str(&commit_beschreibung.to_vec().join("\r\n"));
            no_sig.push_str("\r\n");
        }

        no_sig.push_str(&format!("Hash:         {}\r\n", signatur.hash));
        no_sig.push_str(&format!("Schlüssel-ID: {fingerprint}\r\n"));
        no_sig.push_str("\r\n");

        no_sig.push_str("-----BEGIN PGP SIGNATURE-----\r\n");
        no_sig.push_str(&signatur.pgp_signatur.to_vec().join("\r\n"));
        no_sig.push_str("\r\n-----END PGP SIGNATURE-----\r\n");

        no_sig
    }

    pub fn sync_changes_to_disk(
        upload_changeset: &UploadChangeset,
        folder_path: &PathBuf,
    ) -> Result<(), HttpResponse> {
        let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();

        for neu in upload_changeset.data.neu.iter() {
            let amtsgericht = &neu.analysiert.titelblatt.amtsgericht;
            let grundbuch = &neu.analysiert.titelblatt.grundbuch_von;
            let land = get_land(&gemarkungen, &amtsgericht, &grundbuch)?;

            let blatt = neu.analysiert.titelblatt.blatt;
            let target_path = folder_path
                .clone()
                .join(land.clone())
                .join(amtsgericht)
                .join(grundbuch)
                .join(&format!("{grundbuch}_{blatt}.gbx"));

            let target_json = serde_json::to_string_pretty(&neu).unwrap_or_default();
            let target_folder = folder_path
                .clone()
                .join(land)
                .join(amtsgericht)
                .join(grundbuch);

            let _ = std::fs::create_dir_all(&target_folder);
            let _ = std::fs::write(target_path, target_json.as_bytes());
        }

        for geaendert in upload_changeset.data.geaendert.iter() {
            let amtsgericht = &geaendert.neu.analysiert.titelblatt.amtsgericht;
            let grundbuch = &geaendert.neu.analysiert.titelblatt.grundbuch_von;
            let land = get_land(&gemarkungen, &amtsgericht, &grundbuch)?;

            let blatt = geaendert.neu.analysiert.titelblatt.blatt;
            let target_path = folder_path
                .clone()
                .join(land.clone())
                .join(amtsgericht)
                .join(grundbuch)
                .join(&format!("{grundbuch}_{blatt}.gbx"));

            let target_json = serde_json::to_string_pretty(&geaendert.neu).unwrap_or_default();
            let target_folder = folder_path
                .clone()
                .join(land)
                .join(amtsgericht)
                .join(grundbuch);

            let _ = std::fs::create_dir_all(&target_folder);
            let _ = std::fs::write(target_path, target_json.as_bytes());
        }

        Ok(())
    }

    fn get_land(
        gemarkungen: &GemarkungsBezirke,
        amtsgericht: &str,
        grundbuch: &str,
    ) -> Result<String, HttpResponse> {
        let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
            if ag != amtsgericht {
                return None;
            }
            if bezirk != grundbuch {
                return None;
            }
            Some(land.clone())
        });

        let error = || {
            serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(
                UploadChangesetResponseError {
                    code: 1,
                    text: format!(
                        "Ungültiges Amtsgericht oder ungültige Gemarkung: {}/{}",
                        amtsgericht, grundbuch
                    ),
                },
            ))
            .unwrap_or_default()
        };

        let land = land.ok_or(
            HttpResponse::Ok()
                .content_type("application/json")
                .body(error()),
        )?;

        Ok(land)
    }

    pub async fn commit_changes(
        app_state: &AppState,
        folder_path: &PathBuf,
        benutzer: &BenutzerInfo,
        upload_changeset: &UploadChangeset,
    ) -> Result<(), String> {
        use git2::Repository;

        let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();

        let repo = match Repository::open(&folder_path) {
            Ok(o) => o,
            Err(_) => Repository::init(&folder_path).map_err(|e| format!("{e}"))?,
        };

        let mut index = repo.index().map_err(|e| format!("{e}"))?;
        let _ = index.add_all(["*.gbx"].iter(), git2::IndexAddOption::DEFAULT, None);
        let _ = index.write();

        let signature =
            git2::Signature::now(&benutzer.name, &benutzer.email).map_err(|e| format!("{e}"))?;

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

        let geaendert_blaetter = upload_changeset
            .data
            .geaendert
            .iter()
            .map(|aenderung| {
                let tb = &aenderung.neu.analysiert.titelblatt;
                format!("{}/{}/{}", tb.amtsgericht, tb.grundbuch_von, tb.blatt)
            })
            .collect::<BTreeSet<_>>();

        let (grundbuch_schema, grundbuch_index) = crate::index::get_grundbuch_index()
            .map_err(|e| format!("Fehler in Index / Schema \"grundbuch\": {e}"))?;

        let mut index_writer = grundbuch_index
            .writer(10_000_000)
            .map_err(|e| format!("Fehler bei Allokation von 10MB für Schema \"grundbuch\": {e}"))?;

        for blatt in upload_changeset.data.neu.iter() {
            let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                if *ag == blatt.analysiert.titelblatt.amtsgericht
                    && *bezirk == blatt.analysiert.titelblatt.grundbuch_von
                {
                    Some(land.clone())
                } else {
                    None
                }
            });

            let land = land.ok_or(format!(
                "Kein Land für Grundbuch {}_{}.gbx gefunden",
                blatt.analysiert.titelblatt.grundbuch_von, blatt.analysiert.titelblatt.blatt
            ))?;

            crate::index::add_grundbuchblatt_zu_index(
                &land,
                blatt,
                &index_writer,
                &grundbuch_schema,
            )?;
        }

        for blatt in upload_changeset.data.geaendert.iter() {
            let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                if *ag == blatt.neu.analysiert.titelblatt.amtsgericht
                    && *bezirk == blatt.neu.analysiert.titelblatt.grundbuch_von
                {
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

            crate::index::add_grundbuchblatt_zu_index(
                &land,
                &blatt.neu,
                &index_writer,
                &grundbuch_schema,
            )?;
        }

        let _ = index_writer
            .commit()
            .map_err(|e| format!("Fehler bei index.commit(): {e}"))?;

        for blatt in geaendert_blaetter {
            let webhook_abos = crate::db::get_webhook_abos(&blatt).map_err(|e| format!("{e}"))?;

            for abo_info in webhook_abos {
                let _ = crate::email::send_change_webhook(
                    &app_state.host_name(),
                    &abo_info,
                    &commit_id,
                )
                .await;
            }

            let email_abos = crate::db::get_email_abos(&blatt).map_err(|e| format!("{e}"))?;

            for abo_info in email_abos {
                let _ = crate::email::send_change_email(
                    &app_state.host_name(),
                    &abo_info,
                    &commit_id,
                );
            }
        }

        Ok(())
    }
}

/// API für `/download` Anfragen
pub mod download {

    use crate::models::Grundbuch;
    use crate::models::{get_data_dir, MountPoint, PdfFile};
    use crate::pdf::PdfGrundbuchOptions;
    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};
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
        let (_token, _benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => {
                return e;
            }
        };
        let (amtsgericht, grundbuch_von, blatt) = &*path;
        let mut amtsgericht = amtsgericht.clone();
        let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();

        let mut l = None;
        for (land, ag, bezirk) in gemarkungen.iter() {
            if (amtsgericht == "*" && bezirk == grundbuch_von)
                || (ag.as_str() == amtsgericht.as_str() && bezirk == grundbuch_von)
            {
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

        let folder_path = get_data_dir(MountPoint::Local);
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
            Err(e) => {
                return e;
            }
        };
        let (amtsgericht, grundbuch_von, blatt) = &*path;
        let mut amtsgericht = amtsgericht.clone();
        let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();

        let mut l = None;
        for (land, ag, bezirk) in gemarkungen.iter() {
            if (amtsgericht == "*" && bezirk == grundbuch_von)
                || (ag.as_str() == amtsgericht.as_str() && bezirk == grundbuch_von)
            {
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

        let folder_path = get_data_dir(MountPoint::Local);
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
                return HttpResponse::Ok().content_type("application/json").body(
                    serde_json::to_string_pretty(&PdfFileOrEmpty::NichtVorhanden(
                        PdfFileNichtVorhanden {
                            code: 404,
                            text: format!("Datei für {grundbuch_von}_{blatt}.gbx nicht gefunden"),
                        },
                    ))
                    .unwrap_or_default(),
                );
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

    #[get("/aenderung/pdf/{id}")]
    async fn dowload_aenderung_pdf(
        path: web::Path<String>,
        req: HttpRequest,
    ) -> impl Responder {
        let (_token, _) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => {
                return e;
            }
        };

        let pdf_bytes = generate_diff(&path);

        HttpResponse::Ok()
            .content_type("application/pdf")
            .body(pdf_bytes)
    }

    fn generate_diff(id: &str) -> Vec<u8> {
        use printpdf::{PdfDocument, Mm};
        let titel = format!("Diff with ID {id}");
        let (mut doc, page1, layer1) = PdfDocument::new(&titel, Mm(210.0), Mm(297.0), "Titelblatt");
        doc.save_to_bytes().unwrap_or_default()
    }

    fn generate_pdf(gb: &Grundbuch, options: &PdfGrundbuchOptions) -> Vec<u8> {
        use crate::pdf::PdfFonts;
        use printpdf::Mm;
        use printpdf::PdfDocument;

        let grundbuch_von = gb.titelblatt.grundbuch_von.clone();
        let blatt = gb.titelblatt.blatt;
        let amtsgericht = gb.titelblatt.amtsgericht.clone();

        let titel = format!("{grundbuch_von} Blatt {blatt} (Amtsgericht {amtsgericht})");
        let (mut doc, page1, layer1) = PdfDocument::new(&titel, Mm(210.0), Mm(297.0), "Titelblatt");
        let titelblatt = format!("{}_{}", gb.titelblatt.grundbuch_von, gb.titelblatt.blatt);
        let fonts = PdfFonts::new(&mut doc);

        crate::pdf::write_titelblatt(
            &mut doc.get_page(page1).get_layer(layer1),
            &fonts,
            &gb.titelblatt,
        );
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

    use crate::models::{get_data_dir, AbonnementInfo, MountPoint, Titelblatt};
    use crate::suche::{SuchErgebnisAenderung, SuchErgebnisGrundbuch};
    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use regex::Regex;
    use serde_derive::{Deserialize, Serialize};

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
            Err(e) => {
                return e;
            }
        };
        let folder_path = get_data_dir(MountPoint::Local);
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

        let abos = crate::db::get_abos_fuer_benutzer(&benutzer).unwrap_or_default();

        let grundbuecher = ergebnisse
            .grundbuecher
            .into_iter()
            .filter_map(|ergebnis| {
                let titelblatt = Titelblatt {
                    amtsgericht: ergebnis.amtsgericht.clone(),
                    grundbuch_von: ergebnis.grundbuch_von.clone(),
                    blatt: ergebnis.blatt.parse().ok()?,
                };

                let abos = abos
                    .iter()
                    .filter(|a| {
                        a.amtsgericht == ergebnis.amtsgericht
                            && a.grundbuchbezirk == ergebnis.grundbuch_von
                            && a.blatt.to_string() == ergebnis.blatt
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
}

/// API für `/abo` Anfragen
pub mod abo {

    use super::commit::DbChangeOp;
    use crate::{models::MountPoint, AboLoeschenArgs, AboNeuArgs, AppState};
    use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
    use serde_derive::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "status")]
    enum AboNeuAnfrage {
        #[serde(rename = "ok")]
        Ok(AboNeuAnfrageOk),
        #[serde(rename = "error")]
        Err(AboNeuAnfrageErr),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AboNeuAnfrageOk {}

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AboNeuAnfrageErr {
        code: usize,
        text: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AboNeuForm {
        tag: Option<String>,
    }

    #[get("/abo-neu/{email_oder_webhook}/{amtsgericht}/{grundbuchbezirk}/{blatt}")]
    async fn abo_neu(
        app_state: web::Data<AppState>,
        path: web::Path<(String, String, String, usize)>,
        form: web::Json<AboNeuForm>,
        req: HttpRequest,
    ) -> impl Responder {
        let response_err = |code: usize, text: String| {
            let json = serde_json::to_string_pretty(&AboNeuAnfrage::Err(AboNeuAnfrageErr {
                code: code,
                text: text,
            }))
            .unwrap_or_default();

            HttpResponse::Ok()
                .content_type("application/json")
                .body(json)
        };

        let (token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => {
                return e;
            }
        };
        let (email_oder_webhook, amtsgericht, grundbuchbezirk, blatt) = &*path;

        let abo_return = if app_state.k8s_aktiv() {
            super::write_to_root_db(
                DbChangeOp::AboNeu(AboNeuArgs {
                    typ: email_oder_webhook.clone(),
                    blatt: format!("{amtsgericht}/{grundbuchbezirk}/{blatt}"),
                    text: benutzer.email.clone(),
                    aktenzeichen: form.tag.clone(),
                }),
                &*app_state,
            )
            .await
        } else {
            crate::db::create_abo(
                MountPoint::Local,
                &email_oder_webhook,
                &format!("{amtsgericht}/{grundbuchbezirk}/{blatt}"),
                &benutzer.email,
                form.tag.as_ref().map(|s| s.as_str()),
            )
        };

        match abo_return {
            Ok(()) => HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string_pretty(&AboNeuAnfrage::Ok(AboNeuAnfrageOk {}))
                    .unwrap_or_default(),
            ),
            Err(e) => response_err(500, format!("Fehler beim Erstellen des Abonnements: {e}")),
        }
    }

    #[get("/abo-loeschen/{email_oder_webhook}/{amtsgericht}/{grundbuchbezirk}/{blatt}")]
    async fn abo_loeschen(
        app_state: web::Data<AppState>,
        path: web::Path<(String, String, String, usize)>,
        form: web::Json<AboNeuForm>,
        req: HttpRequest,
    ) -> impl Responder {
        let response_err = |code: usize, text: String| {
            let json = serde_json::to_string_pretty(&AboNeuAnfrage::Err(AboNeuAnfrageErr {
                code: code,
                text: text,
            }))
            .unwrap_or_default();

            HttpResponse::Ok()
                .content_type("application/json")
                .body(json)
        };

        let (_token, benutzer) = match super::get_benutzer_from_httpauth(&req).await {
            Ok(o) => o,
            Err(e) => {
                return e;
            }
        };
        let (email_oder_webhook, amtsgericht, grundbuchbezirk, blatt) = &*path;

        let abo_return = if app_state.k8s_aktiv() {
            super::write_to_root_db(
                DbChangeOp::AboLoeschen(AboLoeschenArgs {
                    typ: email_oder_webhook.clone(),
                    blatt: format!("{amtsgericht}/{grundbuchbezirk}/{blatt}"),
                    text: benutzer.email.clone(),
                    aktenzeichen: form.tag.clone(),
                }),
                &*app_state,
            )
            .await
        } else {
            crate::db::delete_abo(
                MountPoint::Local,
                &email_oder_webhook,
                &format!("{amtsgericht}/{grundbuchbezirk}/{blatt}"),
                &benutzer.email,
                form.tag.as_ref().map(|s| s.as_str()),
            )
        };
        match abo_return {
            Ok(()) => HttpResponse::Ok().content_type("application/json").body(
                serde_json::to_string_pretty(&AboNeuAnfrage::Ok(AboNeuAnfrageOk {}))
                    .unwrap_or_default(),
            ),
            Err(e) => response_err(500, format!("Fehler beim Löschen des Abonnements: {e}")),
        }
    }
}
