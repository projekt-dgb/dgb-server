use actix_web::{get, web, App, HttpServer, Responder, HttpResponse, HttpRequest};
use serde_derive::{Serialize, Deserialize};
use url_encoded_data::UrlEncodedData;
use std::collections::{BTreeMap, BTreeSet};
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref RE: Regex = Regex::new("(\\w*)\\s*(\\d*)").unwrap();
    static ref RE_2: Regex = Regex::new("(\\w*)\\s*Blatt\\s*(\\d*)").unwrap();
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AuthFormData {
    benutzer: String,
    email: String,
    passwort: String,
    #[serde(default)]
    pkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum GrundbuchSucheResponse {
    #[serde(rename = "ok")]
    StatusOk(GrundbuchSucheOk),
    #[serde(rename = "error")]
    StatusErr(GrundbuchSucheError)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrundbuchSucheOk {
    pub ergebnisse: Vec<GrundbuchSucheErgebnis>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Titelblatt {
    pub amtsgericht: String,
    pub grundbuch_von: String,
    pub blatt: usize,
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

// --- SUCHE

fn search_files(dir: &String, suchbegriff: &str) -> Result<Vec<GrundbuchSucheErgebnis>, std::io::Error> {

    use std::fs;

    let mut ergebnisse = Vec::new();
    
    if let Some(s) = normalize_search(suchbegriff) {
        if let Some(file) = 
            fs::read_to_string(Path::new(dir).join(format!("{s}.gbx"))).ok()
            .and_then(|s| serde_json::from_str::<PdfFile>(&s).ok()) {
            
            ergebnisse.push(GrundbuchSucheErgebnis {
                titelblatt: file.titelblatt.clone(),
                ergebnis_text: "".to_string(),
                gefunden_text: "".to_string(),
                download_id: s.clone(),
                score: isize::MAX,
            });
            
            return Ok(ergebnisse);
        }
    }
        
    for entry in fs::read_dir(dir)? {
        
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        
        if !metadata.is_file() {
            continue;
        }
        
        let file: Option<PdfFile> = 
            std::fs::read_to_string(&path).ok()
            .and_then(|s| serde_json::from_str(&s).ok());
        
        let pdf = match file {
            Some(s) => s,
            None => continue,
        };
        
        let grundbuch_json = serde_json::to_string_pretty(&pdf.analysiert).unwrap_or_default();
        
        match sublime_fuzzy::best_match(suchbegriff, &grundbuch_json) {
            Some(m) => {
                let score = m.score();
                ergebnisse.push(GrundbuchSucheErgebnis {
                    titelblatt: pdf.titelblatt.clone(),
                    ergebnis_text: sublime_fuzzy::format_simple(&m, suchbegriff, "<strong>", "</strong>"),
                    gefunden_text: "".to_string(),
                    download_id: format!("{}_{}", pdf.titelblatt.grundbuch_von, pdf.titelblatt.blatt),
                    score: score,
                });
            },
            None => continue,
        }
    }
    
    ergebnisse.sort_by(|a, b| a.score.cmp(&b.score));
    ergebnisse.dedup();
    
    Ok(ergebnisse.into_iter().take(50).collect())
}


fn normalize_search(text: &str) -> Option<String> {
    if RE.is_match(text) {
        let grundbuch_von = RE.find_iter(text).nth(1)?.as_str();
        let blatt = RE.find_iter(text).nth(1).and_then(|s| s.as_str().parse::<usize>().ok())?;
        Some(format!("{grundbuch_von}_{blatt}"))
    } else if RE_2.is_match(text) {
        let grundbuch_von = RE_2.find_iter(text).nth(1)?.as_str();
        let blatt = RE_2.find_iter(text).nth(1).and_then(|s| s.as_str().parse::<usize>().ok())?;
        Some(format!("{grundbuch_von}_{blatt}"))
    } else {
        None
    }
}

#[get("/suche/{suchbegriff}")]
async fn suche(suchbegriff: web::Path<String>, req: HttpRequest) -> impl Responder {
    
    let auth = UrlEncodedData::parse_str(req.query_string());
    let auth = AuthFormData {
        benutzer: auth.get_first("benutzer").map(|s| s.to_string()).unwrap(),
        email: auth.get_first("email").map(|s| s.to_string()).unwrap(),
        passwort: auth.get_first("passwort").map(|s| s.to_string()).unwrap(),
        pkey: auth.get_first("pkey").map(|s| s.to_string()),
    };

    let folder_path = std::env::current_exe().unwrap().join("grundbuchblaetter").to_str().unwrap_or_default().to_string();

    let json = serde_json::to_string_pretty(&GrundbuchSucheResponse::StatusOk(GrundbuchSucheOk {
        ergebnisse: search_files(&folder_path, &*suchbegriff).unwrap_or_default(),
    })).unwrap_or_default();
    
    HttpResponse::Ok().body(json)
}

// --- DOWNLOAD


#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PdfFileOrEmpty {
    Pdf(PdfFile),
    #[serde(rename = "grundbuchNichtInDatenbankVorhanden")]
    NichtVorhanden,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfFile {
    // Pfad der zugehörigen .pdf-Datei
    datei: Option<String>,
    // Some(pfad) wenn Datei digital angelegt wurde
    #[serde(default)]
    gbx_datei_pfad: Option<String>,
    #[serde(default)]
    land: Option<String>,
    titelblatt: Titelblatt,
    seitenzahlen: Vec<u32>,
    geladen: BTreeMap<u32, SeiteParsed>,
    analysiert: Grundbuch,
    pdftotext_layout: PdfToTextLayout,

    /// Seitennummern von Seiten, die versucht wurden, geladen zu werden
    #[serde(default)]
    seiten_versucht_geladen: BTreeSet<u32>,
    #[serde(default)]
    seiten_ocr_text: BTreeMap<u32, String>,
    #[serde(default)]
    anpassungen_seite: BTreeMap<usize, AnpassungSeite>,
    #[serde(default)]
    klassifikation_neu: BTreeMap<usize, SeitenTyp>,
    #[serde(default)]
    nebenbeteiligte_dateipfade: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeiteParsed {
    pub typ: SeitenTyp,
    pub texte: Vec<Vec<Textblock>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Grundbuch {
    pub titelblatt: Titelblatt,
    pub bestandsverzeichnis: Bestandsverzeichnis,
    pub abt1: Abteilung1,
    pub abt2: Abteilung2,
    pub abt3: Abteilung3,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Bestandsverzeichnis {
    // Index = lfd. Nr. der Grundstücke
    pub eintraege: Vec<BvEintrag>,
    pub zuschreibungen: Vec<BvZuschreibung>,
    pub abschreibungen: Vec<BvAbschreibung>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BvEintrag {
    Flurstueck(BvEintragFlurstueck),
    Recht(BvEintragRecht),
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrLines {
    SingleLine(String),
    MultiLine(Vec<String>),
}

impl From<String> for StringOrLines {
    fn from(s: String) -> StringOrLines {
        StringOrLines::MultiLine(
            s.lines()
            .map(|s| s.to_string())
            .collect()
        )
    }
}

impl Default for StringOrLines {
    fn default() -> Self {
        String::new().into()
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BvEintragFlurstueck {
    pub lfd_nr: usize,
    pub bisherige_lfd_nr: Option<usize>,
    pub flur: usize,
    // "87" oder "87/2"
    pub flurstueck: String,
    pub gemarkung: Option<String>,
    pub bezeichnung: Option<StringOrLines>,
    pub groesse: FlurstueckGroesse,
    #[serde(default)]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BvEintragRecht {
    pub lfd_nr: usize,
    pub zu_nr: StringOrLines,
    pub bisherige_lfd_nr: Option<usize>,
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
#[serde(tag = "typ", content = "wert")]
pub enum FlurstueckGroesse {
    #[serde(rename = "m")]
    Metrisch { 
        m2: Option<u64>
    },
    #[serde(rename = "ha")]
    Hektar { 
        ha: Option<u64>, 
        a: Option<u64>, 
        m2: Option<u64>,
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BvZuschreibung {
    pub bv_nr: StringOrLines,
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BvAbschreibung {
    pub bv_nr: StringOrLines,
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Abteilung1 {
    // Index = lfd. Nr. der Grundstücke
    #[serde(default)]
    pub eintraege: Vec<Abt1Eintrag>,
    #[serde(default)]
    pub grundlagen_eintragungen: Vec<Abt1GrundEintragung>,
    #[serde(default)]
    pub veraenderungen: Vec<Abt1Veraenderung>,
    #[serde(default)]
    pub loeschungen: Vec<Abt1Loeschung>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abt1Eintrag {
    // lfd. Nr. der Eintragung
    pub lfd_nr: usize,
    // Rechtstext
    pub eigentuemer: StringOrLines,
    // Used to distinguish from Abt1EintragV1
    pub version: usize,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Abt1GrundEintragung {
    // lfd. Nr. der Eintragung
    pub bv_nr: StringOrLines,
    // Grundlage der Eintragung
    pub text: StringOrLines,
    
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt1Veraenderung {
    pub lfd_nr: StringOrLines,
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt1Loeschung {
    pub lfd_nr: StringOrLines,
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Abteilung2 {
    // Index = lfd. Nr. der Grundstücke
    pub eintraege: Vec<Abt2Eintrag>,
    pub veraenderungen: Vec<Abt2Veraenderung>,
    pub loeschungen: Vec<Abt2Loeschung>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abt2Eintrag {
    // lfd. Nr. der Eintragung
    pub lfd_nr: usize,
    // lfd. Nr der betroffenen Grundstücke im Bestandsverzeichnis
    pub bv_nr: StringOrLines, // Vec<BvNr>,
    // Rechtstext
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt2Veraenderung {
    pub lfd_nr: StringOrLines,
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt2Loeschung {
    pub lfd_nr: StringOrLines,
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Abteilung3 {
    // Index = lfd. Nr. der Grundstücke
    pub eintraege: Vec<Abt3Eintrag>,
    pub veraenderungen: Vec<Abt3Veraenderung>,
    pub loeschungen: Vec<Abt3Loeschung>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abt3Eintrag {
    // lfd. Nr. der Eintragung
    pub lfd_nr: usize,
    // lfd. Nr der betroffenen Grundstücke im Bestandsverzeichnis
    pub bv_nr: StringOrLines, // Vec<BvNr>,
    // Betrag (EUR / DM)
    pub betrag: StringOrLines,
    /// Rechtstext
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt3Veraenderung {
    pub lfd_nr: StringOrLines,
    #[serde(default)]
    pub betrag: StringOrLines,
    #[serde(default)]
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt3Loeschung {
    pub lfd_nr: StringOrLines,
    #[serde(default)]
    pub betrag: StringOrLines,
    #[serde(default)]
    pub text: StringOrLines,
    #[serde(default)]
    pub automatisch_geroetet: bool,
    #[serde(default)]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct PositionInPdf {
    pub seite: u32,
    pub rect: OptRect,
}

#[derive(Debug, Default, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct OptRect {
    pub min_x: Option<f32>,
    pub max_x: Option<f32>,
    pub min_y: Option<f32>,
    pub max_y: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Textblock {
    pub text: String,
    pub start_y: f32,
    pub end_y: f32,
    pub start_x: f32,
    pub end_x: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnpassungSeite {
    pub spalten: BTreeMap<String, Rect>,    
    pub zeilen: Vec<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Rect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
pub enum SeitenTyp {
    
    #[serde(rename = "bv-horz")]
    BestandsverzeichnisHorz,
    #[serde(rename = "bv-horz-zu-und-abschreibungen")]
	BestandsverzeichnisHorzZuUndAbschreibungen,
    #[serde(rename = "bv-vert")]
    BestandsverzeichnisVert,
    #[serde(rename = "bv-vert-typ2")]
    BestandsverzeichnisVertTyp2,
    #[serde(rename = "bv-vert-zu-und-abschreibungen")]
	BestandsverzeichnisVertZuUndAbschreibungen,
	
    #[serde(rename = "abt1-horz")]
	Abt1Horz,
    #[serde(rename = "abt1-vert")]
	Abt1Vert,
	
    #[serde(rename = "abt2-horz-veraenderungen")]
	Abt2HorzVeraenderungen,
    #[serde(rename = "abt2-horz")]
	Abt2Horz,
    #[serde(rename = "abt2-vert-veraenderungen")]
	Abt2VertVeraenderungen,
    #[serde(rename = "abt2-vert")]
	Abt2Vert,

    #[serde(rename = "abt3-horz-veraenderungen-loeschungen")]
    Abt3HorzVeraenderungenLoeschungen,
    #[serde(rename = "abt3-vert-veraenderungen-loeschungen")]
    Abt3VertVeraenderungenLoeschungen,
    #[serde(rename = "abt3-horz")]
	Abt3Horz,
    #[serde(rename = "abt3-vert-veraenderungen")]
	Abt3VertVeraenderungen,
    #[serde(rename = "abt3-vert-loeschungen")]
	Abt3VertLoeschungen,
    #[serde(rename = "abt3-vert")]
	Abt3Vert,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PdfToTextLayout {
    pub seiten: BTreeMap<u32, PdfToTextSeite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfToTextSeite {
    pub breite_mm: f32,
    pub hoehe_mm: f32,
    pub texte: Vec<Textblock>,
}

#[get("/download/{download_id}")]
async fn download(download_id: web::Path<String>, req: HttpRequest) -> impl Responder {
    
    let auth = UrlEncodedData::parse_str(req.query_string());
    let auth = AuthFormData {
        benutzer: auth.get_first("benutzer").map(|s| s.to_string()).unwrap(),
        email: auth.get_first("email").map(|s| s.to_string()).unwrap(),
        passwort: auth.get_first("passwort").map(|s| s.to_string()).unwrap(),
        pkey: auth.get_first("pkey").map(|s| s.to_string()),
    };

    let folder_path = std::env::current_exe().unwrap().join("grundbuchblaetter");
    let file_id = format!("{download_id}.gbx");
    let file_path = folder_path.join(file_id);
    let file: Option<PdfFile> = std::fs::read_to_string(&file_path).ok().and_then(|s| serde_json::from_str(&s).ok());
    
    let response_json = match file {
        Some(s) => PdfFileOrEmpty::Pdf(s),
        None => PdfFileOrEmpty::NichtVorhanden,
    };
    
    HttpResponse::Ok()
    .body(serde_json::to_string_pretty(&response_json).unwrap_or_default())
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
        .service(suche)
        .service(download)
    })
    .bind(("127.0.0.1", 80))?
    .run()
    .await
}
