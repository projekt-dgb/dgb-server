//! Datenmodelle, die vom Server verarbeitet werden

use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};


pub fn get_db_path() -> String {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("benutzer.sqlite.db")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_data_dir() -> String {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("data")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_keys_dir() -> String {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("keys")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_index_dir() -> String {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("index")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

/// Entspricht einem PDF-Grundbuch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfFile {
    /// Pfad der zugehörigen .pdf-Datei
    #[serde(default)]
    pub datei: Option<String>,
    /// Some(pfad) wenn Datei digital angelegt wurde
    #[serde(default)]
    pub gbx_datei_pfad: Option<String>,
    /// Land des Grundbuchs (z.B. "Brandenburg")
    #[serde(default)]
    pub land: Option<String>,
    /// Titelseite des Grundbuchs (Deckblatt)
    pub titelblatt: Titelblatt,
    /// Seitenzahlen im Grundbuch
    #[serde(default)]
    pub seitenzahlen: Vec<u32>,
    /// Seiten, die geladen wurden
    #[serde(default)]
    pub geladen: BTreeMap<u32, SeiteParsed>,
    /// Analysiertes Grundbuch
    #[serde(default)]
    pub analysiert: Grundbuch,
    /// Layout der Seiten aus pdftotext
    #[serde(default)]
    pub pdftotext_layout: PdfToTextLayout,
    /// Seitennummern von Seiten, die geladen wurden
    #[serde(default)]
    pub seiten_versucht_geladen: BTreeSet<u32>,
    /// Seiten -> OCR Text (tesseract)
    #[serde(default)]
    pub seiten_ocr_text: BTreeMap<u32, String>,
    /// Anpassungen in Zeilen und Spaltengrößen auf der Seite
    #[serde(default)]
    pub anpassungen_seite: BTreeMap<usize, AnpassungSeite>,
    /// Anpassungen im Seitentyp
    #[serde(default)]
    pub klassifikation_neu: BTreeMap<usize, SeitenTyp>,
    /// Dateipfade zu .csv-Dateien für Nebenbeteiligte
    #[serde(default)]
    pub nebenbeteiligte_dateipfade: Vec<String>,
}

/// Deckblatt des PDFs
#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Ord, Eq, Hash, Serialize, Deserialize)]
pub struct Titelblatt {
    /// "Amtsgericht von X"
    pub amtsgericht: String,
    /// "Grundbuch von X"
    pub grundbuch_von: String,
    /// "Blatt X"
    pub blatt: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeiteParsed {
    // Typ der Seite (automatisch erkannt)
    pub typ: SeitenTyp,
    // Textblöcke auf der Seite
    pub texte: Vec<Vec<Textblock>>,
}

/// Analysiertes Grundbuch
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
        StringOrLines::MultiLine(s.lines().map(|s| s.to_string()).collect())
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
    Metrisch { m2: Option<u64> },
    #[serde(rename = "ha")]
    Hektar {
        ha: Option<u64>,
        a: Option<u64>,
        m2: Option<u64>,
    },
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthFormData {
    // E-Mail des Nutzers
    pub email: String,
    // Passwort (Plaintext)
    pub passwort: String,
    // Privater Schlüssel in Base64-Format
    #[serde(default)]
    pub pkey: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenutzerInfo {
    pub id: i32,
    pub rechte: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbonnementInfo {
    pub amtsgericht: String,
    pub grundbuchbezirk: String,
    pub blatt: i32,
    pub text: String,
    pub aktenzeichen: String,
    pub commit_id: String,
}
