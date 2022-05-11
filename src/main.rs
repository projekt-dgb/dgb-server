use actix_web::{get, post, web, App, HttpServer, Responder, HttpResponse, HttpRequest};
use serde_derive::{Serialize, Deserialize};
use url_encoded_data::UrlEncodedData;
use std::collections::{BTreeMap, BTreeSet};
use lazy_static::lazy_static;
use regex::Regex;
use std::path::{Path, PathBuf};
use rusqlite::{params, OpenFlags, Connection};
use rsa::{RsaPrivateKey, pkcs1::LineEnding, PublicKey, pkcs8::{EncodePrivateKey, DecodePrivateKey}, RsaPublicKey, PaddingScheme};
use clap::Parser;
use std::str::FromStr;

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

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize)]
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

fn search_files(dir: &PathBuf, suchbegriff: &str) -> Result<Vec<GrundbuchSucheErgebnis>, std::io::Error> {

    use std::fs;

    let mut ergebnisse = Vec::new();
        
    if let Some(s) = normalize_search(suchbegriff) {
        if let Some(file) = fs::read_to_string(Path::new(dir).join(format!("{s}.gbx"))).ok()
            .and_then(|s| serde_json::from_str::<PdfFile>(&s).ok()) {
            
            ergebnisse.push(GrundbuchSucheErgebnis {
                titelblatt: file.titelblatt.clone(),
                ergebnis_text: "".to_string(),
                gefunden_text: "".to_string(),
                download_id: format!("{}/{}/{}.gbx", file.titelblatt.amtsgericht, file.titelblatt.grundbuch_von, file.titelblatt.blatt),
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
    
        let file: Option<PdfFile> = 
            std::fs::read_to_string(&path).ok()
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
                            let mut t = if i > 0 { format!("{}\r\n<br/>", lines.get(i - 1).cloned().unwrap_or_default()) } else { String::new() };
                            t.push_str(&format!("{line}\r\n"));
                            if let Some(next) = lines.get(i + 1) {
                                t.push_str(&format!("<br/>{next}\r\n"));
                            }
                            t
                        },
                        gefunden_text: suchbegriff.to_string(),
                        download_id: format!("{}/{}/{}.gbx", pdf.titelblatt.amtsgericht, pdf.titelblatt.grundbuch_von, pdf.titelblatt.blatt),
                        score: score,
                    });
                    break;
                },
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
        let grundbuch_von = RE_2.captures_iter(text).nth(0).and_then(|cap| Some(cap.get(1)?.as_str().to_string()))?;
        let blatt = RE_2.captures_iter(text).nth(0).and_then(|cap| Some(cap.get(2)?.as_str().parse::<usize>().ok()?))?;
        Some(format!("{grundbuch_von}_{blatt}"))
    } else if RE.is_match(text) {
        let grundbuch_von = RE.captures_iter(text).nth(0).and_then(|cap| Some(cap.get(1)?.as_str().to_string()))?;
        let blatt = RE.captures_iter(text).nth(0).and_then(|cap| Some(cap.get(2)?.as_str().parse::<usize>().ok()?))?;
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

    let folder_path = std::env::current_exe().unwrap().parent().unwrap().join("data").to_str().unwrap_or_default().to_string();

    let mut ergebnisse = Vec::new();
    
    // /data
    if let Ok(e) = std::fs::read_dir(&folder_path) {
        for entry in e {

            let entry = match entry { Ok(o) => o, _ => continue, };
            let path = entry.path();
            if !path.is_dir() { continue; }
                
            // /data/Prenzlau
            if let Ok(e) = std::fs::read_dir(&path) {
                for entry in e {
                
                    let entry = match entry { Ok(o) => o, _ => continue, };
                    let path = entry.path();
                    if !path.is_dir() { continue; }
                    
                    // /data/Prenzlau/Ludwigsburg
                    if let Ok(mut e) = search_files(&path, &suchbegriff) {
                        ergebnisse.append(&mut e);
                    }
                }   
            }
        }
    }
    
    
    let json = serde_json::to_string_pretty(&GrundbuchSucheResponse::StatusOk(GrundbuchSucheOk {
            ergebnisse,
    })).unwrap_or_default();
    
    HttpResponse::Ok()
    .content_type("application/json")
    .body(json)
}

// --- DOWNLOAD


#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "status")]
pub enum PdfFileOrEmpty {
    #[serde(rename = "ok")]
    Pdf(PdfFile),
    #[serde(rename = "error")]
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
    #[serde(skip, default)]
    icon: Option<PdfFileIcon>,
    
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum PdfFileIcon {
    // Gelbes Warn-Icon
    HatFehler,
    // Halb-grünes Icon
    KeineOrdnungsnummernZugewiesen,
    // Voll-grünes Icon
    AllesOkay,
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
    #[serde(default)]
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

// Eintrag für ein grundstücksgleiches Recht
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
#[serde(untagged)]
#[repr(C)]
pub enum Abt1Eintrag {
    V1(Abt1EintragV1),
    V2(Abt1EintragV2),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abt1EintragV2 {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abt1EintragV1 {
    // lfd. Nr. der Eintragung
    pub lfd_nr: usize,
    // Rechtstext
    pub eigentuemer: StringOrLines,
    // lfd. Nr der betroffenen Grundstücke im Bestandsverzeichnis
    pub bv_nr: StringOrLines, 
    // Vec<BvNr>,
    pub grundlage_der_eintragung: StringOrLines,
    
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

#[get("/download/{amtsgericht}/{grundbuch_von}/{blatt}.gbx")]
async fn download(path: web::Path<(String, String, usize)>, req: HttpRequest) -> impl Responder {
    
    let auth = UrlEncodedData::parse_str(req.query_string());
    let auth = AuthFormData {
        benutzer: auth.get_first("benutzer").map(|s| s.to_string()).unwrap(),
        email: auth.get_first("email").map(|s| s.to_string()).unwrap(),
        passwort: auth.get_first("passwort").map(|s| s.to_string()).unwrap(),
        pkey: auth.get_first("pkey").map(|s| s.to_string()),
    };

    let folder_path = std::env::current_exe().unwrap().parent().unwrap().join("data");
    let (amtsgericht, grundbuch_von, blatt) = &*path;
    
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
            std::fs::read_to_string(&file_path).ok().and_then(|s| serde_json::from_str(&s).ok()) 
        })
        
    } else {
        let file_path = folder_path.join(amtsgericht).join(grundbuch_von).join(&format!("{grundbuch_von}_{blatt}.gbx"));
        std::fs::read_to_string(&file_path).ok().and_then(|s| serde_json::from_str(&s).ok()) 
    };

    let response_json = match file {
        Some(s) => PdfFileOrEmpty::Pdf(s),
        None => PdfFileOrEmpty::NichtVorhanden,
    };
    
    HttpResponse::Ok()
    .content_type("application/json")
    .body(serde_json::to_string_pretty(&response_json).unwrap_or_default())
}

// --- UPLOAD

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
pub enum UploadChangesetResponse {
    StatusOk(UploadChangesetResponseOk),
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
async fn upload(upload_changeset: web::Json<UploadChangeset>, req: HttpRequest) -> impl Responder {
    
    let auth = UrlEncodedData::parse_str(req.query_string());
    let auth = AuthFormData {
        benutzer: auth.get_first("benutzer").map(|s| s.to_string()).unwrap(),
        email: auth.get_first("email").map(|s| s.to_string()).unwrap(),
        passwort: auth.get_first("passwort").map(|s| s.to_string()).unwrap(),
        pkey: auth.get_first("pkey").map(|s| s.to_string()),
    };
    
    let folder_path = std::env::current_exe().unwrap().parent().unwrap().join("data");
    
    if !folder_path.exists() {
        let _ = std::fs::create_dir(folder_path.clone());
    }
    
    let mut check = UploadChangesetResponseOk {
        neu: BTreeMap::new(),
        geaendert: BTreeMap::new(),
    };

    for (name, neu) in upload_changeset.neue_dateien.iter() {
        
        let amtsgericht = &neu.titelblatt.amtsgericht;
        let grundbuch = &neu.titelblatt.grundbuch_von;
        let blatt = neu.titelblatt.blatt;
        let target_path = folder_path.clone().join(amtsgericht).join(grundbuch).join(&format!("{grundbuch}_{blatt}.gbx"));
        let target_json = serde_json::to_string_pretty(&neu).unwrap_or_default();
        let target_folder = folder_path.clone().join(amtsgericht).join(grundbuch);

        let _ = std::fs::create_dir_all(&target_folder);
        let _ = std::fs::write(target_path, target_json.as_bytes());
        
        check.neu.insert(name.clone(), neu.clone());
    }
    
    for (name, geaendert) in upload_changeset.geaenderte_dateien.iter() {
        
        let amtsgericht = &geaendert.neu.titelblatt.amtsgericht;
        let grundbuch = &geaendert.neu.titelblatt.grundbuch_von;
        let blatt = geaendert.neu.titelblatt.blatt;
        let target_path = folder_path.clone().join(amtsgericht).join(grundbuch).join(&format!("{grundbuch}_{blatt}.gbx"));
        let target_json = serde_json::to_string_pretty(&geaendert.neu).unwrap_or_default();
        let target_folder = folder_path.clone().join(amtsgericht).join(grundbuch);

        let _ = std::fs::create_dir_all(&target_folder);
        let _ = std::fs::write(target_path, target_json.as_bytes());
        
        check.geaendert.insert(name.clone(), geaendert.neu.clone());
    }
    
    if !check.geaendert.is_empty() ||
       !check.neu.is_empty() {
        
        use git2::Repository;

        let repo = match Repository::open(&folder_path) {
            Ok(repo) => repo,
            Err(_) => {
                match Repository::init(&folder_path) {
                    Ok(repo) => repo,
                    Err(e) => {
                        return HttpResponse::Ok()
                        .content_type("application/json")
                        .body(serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(UploadChangesetResponseError {
                            code: 1,
                            text: format!("Konnte Änderungstext nicht speichern"),
                        })).unwrap_or_default());
                    },
                }
            },
        };
        
        let mut index = match repo.index() {
            Ok(o) => o,
            Err(e) => {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(UploadChangesetResponseError {
                    code: 2,
                    text: format!("Konnte Änderungstext nicht speichern"),
                })).unwrap_or_default());
            }
        };
        
        let _ = index.add_all(["*.gbx"].iter(), git2::IndexAddOption::DEFAULT, None);
        let _ = index.write();
        
        let signature = match git2::Signature::now(&auth.benutzer, &auth.email) {
            Ok(o) => o,
            Err(e) => {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(UploadChangesetResponseError {
                    code: 4,
                    text: format!("Konnte Änderungstext nicht speichern"),
                })).unwrap_or_default());
            }
        };
        
        let msg = format!("{}\r\n\r\n{}", upload_changeset.aenderung_titel.trim(), upload_changeset.aenderung_beschreibung);
        
        let id = match index.write_tree() {
            Ok(o) => o,
            Err(e) => {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(UploadChangesetResponseError {
                    code: 5,
                    text: format!("Konnte Änderungstext nicht speichern"),
                })).unwrap_or_default());
            }
        };
        
        let tree = match repo.find_tree(id) {
            Ok(o) => o,
            Err(e) => {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(UploadChangesetResponseError {
                    code: 6,
                    text: format!("Konnte Änderungstext nicht speichern"),
                })).unwrap_or_default());
            }
        };
        
        
        let parent = repo.head().ok()
            .and_then(|c| c.target())
            .and_then(|head_target| repo.find_commit(head_target).ok());
        
        let parents = match parent.as_ref() {
            Some(s) => vec![s],
            None => Vec::new(),
        };
        
        let commit = match repo.commit(
            Some("HEAD"), 
            &signature, 
            &signature, 
            &msg, 
            &tree,
            &parents,
        ) {
            Ok(o) => o,
            Err(e) => {
                return HttpResponse::Ok()
                .content_type("application/json")
                .body(serde_json::to_string_pretty(&UploadChangesetResponse::StatusError(UploadChangesetResponseError {
                    code: 9,
                    text: format!("Konnte Änderungstext nicht speichern"),
                })).unwrap_or_default());
            }
        };
    }
    
    HttpResponse::Ok()
    .content_type("application/json")
    .body(serde_json::to_string_pretty(&UploadChangesetResponse::StatusOk(check)).unwrap_or_default())
}

fn get_db_path() -> String {
    std::env::current_exe().unwrap()
    .parent().unwrap()
    .join("benutzer.sqlite.db").to_str()
    .unwrap_or_default().to_string()
}

fn create_database() -> Result<(), rusqlite::Error> {

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

fn create_gemarkung(land: &str, amtsgericht: &str, bezirk: &str) -> Result<(), String> {

    let conn = Connection::open(get_db_path())
    .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "INSERT INTO bezirke (land, amtsgericht, bezirk) VALUES (?1, ?2, ?3)",
        params![land, amtsgericht, bezirk],
    ).map_err(|e| format!("Fehler beim Einfügen von {land}/{amtsgericht}/{bezirk} in Datenbank: {e}"))?;
    
    Ok(())
}

fn delete_gemarkung(land: &str, amtsgericht: &str, bezirk: &str) -> Result<(), String> {

    let conn = Connection::open(get_db_path())
    .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "DELETE FROM bezirke WHERE land = ?1 AND amtsgericht = ?2 AND bezirk = ?3",
        params![land, amtsgericht, bezirk],
    ).map_err(|e| format!("Fehler beim Löschen von {land}/{amtsgericht}/{bezirk} in Datenbank: {e}"))?;
    
    Ok(())
}

fn create_user(name: &str, email: &str, passwort: &str, rechte: &str) -> Result<(), String> {

    let mut rng = rand::thread_rng();
    let bits = 2048;

    let private_key = RsaPrivateKey::new(&mut rng, bits)
    .map_err(|e| format!("Fehler in RsaPrivateKey::new: {e}"))?;
    
    let public_key = RsaPublicKey::from(&private_key);
    
    if passwort.len() > 50 {
        return Err(format!("Passwort zu lang"));
    }
    
    let password_hashed = public_key
        .encrypt(&mut rng, PaddingScheme::new_pkcs1v15_encrypt(), passwort.as_bytes())
        .map_err(|e| format!("Konnte Passwort nicht verschlüsseln: {e}"))?;

    let conn = Connection::open(get_db_path())
    .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;
    
    private_key.write_pkcs8_pem_file(format!("./{email}.pem"), LineEnding::CRLF)
    .map_err(|e| format!("Fehler beim Schreiben von {email}.pem: {e}"))?;
    
    conn.execute(
        "INSERT INTO benutzer (email, name, rechte, password_hashed) VALUES (?1, ?2, ?3, ?4)",
        params![email, name, rechte, password_hashed],
    ).map_err(|e| format!("Fehler beim Einfügen von {email} in Datenbank: {e}"))?;
    
    Ok(())
}

fn validate_user(name: &str, email: &str, passwort: &str, private_key: &str) -> Result<String, String> {

    let private_key = RsaPrivateKey::from_pkcs8_pem(&private_key)
    .map_err(|e| format!("Ungültiger privater Schlüssel"))?;
    
    let conn = Connection::open(get_db_path())
    .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;
    
    let mut stmt = conn.prepare(
        "SELECT id, rolle, password_hashed FROM benutzer WHERE email = ?1 AND name = ?2"
    ).map_err(|e| format!("Fehler beim Auslesen der Benutzerdaten"))?;
        
    let benutzer = stmt.query_map(params![email, name], |row| { Ok((row.get::<usize, i32>(0)?, row.get::<usize, String>(1)?, row.get::<usize, Vec<u8>>(2)?)) })
        .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?
        .collect::<Vec<_>>();
    
    let (id, rolle, pw) = match benutzer.get(0) {
        Some(Ok((id, rolle, pw))) => (id, rolle, pw),
        _ => return Err(format!("Kein Benutzer für \"{name} <{email}>\" gefunden")),
    };
    
    let passwort_decrypted = private_key.decrypt(PaddingScheme::new_pkcs1v15_encrypt(), pw.as_ref())
        .map_err(|e| format!("Passwort falsch: Privater Schlüssel ungültig für {email}"))?;
    
    let passwort_decrypted = String::from_utf8(passwort_decrypted)
        .map_err(|e| format!("Konnte Passwort nicht entschlüsseln"))?;
        
    if passwort_decrypted != passwort {
        return Err(format!("Ungültiges Passwort"));
    }
    
    Ok(rolle.clone())
}

fn delete_user(email: &str) -> Result<(), String> {
    let conn = Connection::open(get_db_path())
    .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;
    
    conn.execute("DELETE FROM benutzer WHERE email = ?1", params![email])
    .map_err(|e| format!("Fehler beim Löschen von {email}: {e}"))?;
    
    Ok(())
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Ob der Server HTTPS benutzen soll
    #[clap(short, long)]
    https: bool,
    
    /// Server-interne IP, auf der der Server erreichbar sein soll
    #[clap(short, long, default_value = "127.0.0.1")]
    ip: String,
    
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
    
    /// Neues Abonnement anlegen (--email, --aktenzeichen)
    AboNeu(AboNeuArgs),
    /// Abonnement löschen (--email, --aktenzeichen)
    AboLoeschen(AboLoeschenArgs),
}

#[derive(Parser, Debug, PartialEq)]
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

#[derive(Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
struct BenutzerLoeschenArgs {

    /// E-Mail des Benutzers, der gelöscht werden soll
    #[clap(short, long)]
    email: String,
}

#[derive(Parser, Debug, PartialEq)]
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

#[derive(Parser, Debug, PartialEq)]
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

#[derive(Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
struct AboNeuArgs {
    /// Name des Amtsgerichts / Gemarkung / Blatts des neuen Abos, getrennt mit Schrägstrich ("Prenzlau / Ludwigsburg / 254 ")
    #[clap(short, long)]
    blatt: String,
    
    /// Name der E-Mail, für die das Abo eingetragen werden soll
    #[clap(short, long)]
    email: String,
    
    /// Aktenzeichen für das neue Abo
    #[clap(short, long)]
    aktenzeichen: String,
}

#[derive(Parser, Debug, PartialEq)]
#[clap(author, version, about, long_about = None)]
struct AboLoeschenArgs {
    
    /// Name des Amtsgerichts / Gemarkung / Blatts des Abos, getrennt mit Schrägstrich ("Prenzlau / Ludwigsburg / 254 ")
    #[clap(short, long)]
    blatt: String,
    
    /// Name der E-Mail, für die das Abo eingetragen ist
    #[clap(short, long)]
    email: String,
    
    /// Aktenzeichen des Abonnements
    #[clap(short, long)]
    aktenzeichen: String,
}

fn create_abo(blatt: &str, email: &str, aktenzeichen: &str) -> Result<(), String> {
    
    let blatt_split = blatt
        .split("/")
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();
    
    let amtsgericht = match blatt_split.get(0) {
        Some(s) => s.trim().to_string(),
        None => { return Err(format!("Kein Amtsgericht angegeben für Abonnement {blatt}")); },
    };
    
    let bezirk = match blatt_split.get(1) {
        Some(s) => s.trim().to_string(),
        None => { return Err(format!("Kein Bezirk angegeben für Abonnement {blatt}")); },
    };
    
    let b = match blatt_split.get(2) {
        Some(s) => s.trim().parse::<i32>().map_err(|e| format!("Ungültige Blatt-Nr. {s}: {e}"))?,
        None => { return Err(format!("Kein Blatt angegeben für Abonnement {blatt}")); },
    };
    
    let conn = Connection::open(get_db_path())
    .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;
    
    conn.execute(
        "INSERT INTO abonnements (email, amtsgericht, bezirk, blatt, aktenzeichen) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![email, amtsgericht, bezirk, b, aktenzeichen],
    ).map_err(|e| format!("Fehler beim Einfügen von {blatt} in Abonnements: {e}"))?;
    
    Ok(())
}

fn delete_abo(blatt: &str, email: &str, aktenzeichen: &str) -> Result<(), String> {
    
    let blatt_split = blatt
        .split("/")
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();
    
    let amtsgericht = match blatt_split.get(0) {
        Some(s) => s.trim().to_string(),
        None => { return Err(format!("Kein Amtsgericht angegeben für Abonnement {blatt}")); },
    };
    
    let bezirk = match blatt_split.get(1) {
        Some(s) => s.trim().to_string(),
        None => { return Err(format!("Kein Bezirk angegeben für Abonnement {blatt}")); },
    };
    
    let b = match blatt_split.get(2) {
        Some(s) => s.trim().parse::<i32>().map_err(|e| format!("Ungültige Blatt-Nr. {s}: {e}"))?,
        None => { return Err(format!("Kein Blatt angegeben für Abonnement {blatt}")); },
    };
    
    let conn = Connection::open(get_db_path())
    .map_err(|e| format!("Fehler bei Verbindung zur Benutzerdatenbank"))?;

    conn.execute(
        "DELETE FROM abonnements WHERE email = ?1 AND amtsgericht = ?2 AND bezirk = ?3 AND blatt = ?4 AND aktenzeichen = ?5",
        params![email, amtsgericht, bezirk, b, aktenzeichen],
    ).map_err(|e| format!("Fehler beim Löschen von {blatt} in Abonnements: {e}"))?;
    
    Ok(())
}

fn process_action(action: &ArgAction) -> Result<(), String> {
    use self::ArgAction::*;
    match action {
        BenutzerNeu(BenutzerNeuArgs { name, email, passwort, rechte }) => create_user(name, email, passwort, rechte),
        BenutzerLoeschen(BenutzerLoeschenArgs { email }) => delete_user(email),
        BezirkNeu(BezirkNeuArgs { land, amtsgericht, bezirk}) => create_gemarkung(land, amtsgericht, bezirk),
        BezirkLoeschen(BezirkLoeschenArgs { land, amtsgericht, bezirk }) => delete_gemarkung(land, amtsgericht, bezirk),
        AboNeu(AboNeuArgs { blatt, email, aktenzeichen }) => create_abo(blatt, email, aktenzeichen),
        AboLoeschen(AboLoeschenArgs { blatt, email, aktenzeichen }) => delete_abo(blatt, email, aktenzeichen),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {

    let args = Args::parse();
    
    if let Err(e) = create_database() {
        println!("Fehler in create_database: {e}");
        return Ok(());
    }
            
    match args.action.as_ref() {
        Some(s) => {
            if let Err(e) = process_action(s) {
                println!("{s:?}: {e}");
            }
            return Ok(());
        },
        None => { },
    }
    
    HttpServer::new(|| {

        let json_cfg = web::JsonConfig::default()
        .limit(usize::MAX)
        .content_type_required(false);
        
        App::new()
        .app_data(json_cfg)
        .service(suche)
        .service(download)
        .service(upload)
    })
    .bind((args.ip, if args.https { 443 } else { 80 }))?
    .run()
    .await
}
