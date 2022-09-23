//! Datenmodelle, die vom Server verarbeitet werden

use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum MountPoint {
    Local,
    Remote,
}

pub fn get_local_path() -> String {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("local")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_remote_path() -> String {
    std::env::var("REMOTE_MOUNT_POINT").unwrap_or("/mnt/data/files".to_string())
}

pub fn get_base_path(mount_point: MountPoint) -> String {
    match mount_point {
        MountPoint::Local => get_local_path(),
        MountPoint::Remote => get_remote_path(),
    }
}

pub fn get_db_path(mount_point: MountPoint) -> String {
    Path::new(&get_base_path(mount_point))
        .join("benutzer.sqlite.db")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_data_dir(mount_point: MountPoint) -> String {
    Path::new(&get_base_path(mount_point))
        .join("data")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_index_dir() -> String {
    Path::new(&get_base_path(MountPoint::Local))
        .join("index")
        .to_str()
        .unwrap_or_default()
        .to_string()
}

/// Entspricht einem PDF-Grundbuch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfFile {
    #[serde(default)]
    pub digitalisiert: bool,
    // Pfad der zugehörigen .pdf-Datei
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datei: Option<String>,
    /// Some(pfad) wenn Datei digital angelegt wurde
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gbx_datei_pfad: Option<String>,
    /// Land des Grundbuchs (z.B. "Brandenburg")
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub land: Option<String>,
    /// Seitenzahlen im Grundbuch
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub seitenzahlen: Vec<u32>,
    /// Seiten, die geladen wurden
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub geladen: BTreeMap<String, SeiteParsed>,
    /// Layout der Seiten aus pdftotext
    #[serde(default)]
    #[serde(skip_serializing_if = "PdfToTextLayout::is_empty")]
    pub pdftotext_layout: PdfToTextLayout,
    /// Seitennummern von Seiten, die versucht wurden, geladen zu werden
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub seiten_versucht_geladen: BTreeSet<u32>,
    /// Seiten -> OCR Text (tesseract)
    #[serde(default)]
    pub seiten_ocr_text: BTreeMap<String, String>,
    /// Anpassungen in Zeilen und Spaltengrößen auf der Seite
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub anpassungen_seite: BTreeMap<String, AnpassungSeite>,
    /// Anpassungen im Seitentyp
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    pub klassifikation_neu: BTreeMap<String, SeitenTyp>,
    /// Dateipfade zu .csv-Dateien für Nebenbeteiligte
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nebenbeteiligte_dateipfade: Vec<String>,
    /// Analysiertes Grundbuch
    #[serde(alias = "inhalt")]
    pub analysiert: Grundbuch,
}

/// Deckblatt des PDFs
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Titelblatt {
    /// Amtsgericht von X
    pub amtsgericht: String,
    /// Grundbuch von X
    pub grundbuch_von: String,
    /// Blatt X
    pub blatt: StringOrUsize,
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrUsize {
    S(String),
    U(usize),
}

impl std::fmt::Display for StringOrUsize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            StringOrUsize::S(s) => write!(f, "{s}"),
            StringOrUsize::U(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeiteParsed {
    // Typ der Seite (automatisch erkannt)
    pub typ: SeitenTyp,
    // Textblöcke auf der Seite
    pub texte: Vec<Vec<Textblock>>,
}

/// Analysiertes Grundbuch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grundbuch {
    pub titelblatt: Titelblatt,
    #[serde(default)]
    #[serde(skip_serializing_if = "Bestandsverzeichnis::is_empty")]
    pub bestandsverzeichnis: Bestandsverzeichnis,
    #[serde(default)]
    #[serde(skip_serializing_if = "Abteilung1::is_empty")]
    pub abt1: Abteilung1,
    #[serde(default)]
    #[serde(skip_serializing_if = "Abteilung2::is_empty")]
    pub abt2: Abteilung2,
    #[serde(default)]
    #[serde(skip_serializing_if = "Abteilung3::is_empty")]
    pub abt3: Abteilung3,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Bestandsverzeichnis {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub eintraege: Vec<BvEintrag>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub zuschreibungen: Vec<BvZuschreibung>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub abschreibungen: Vec<BvAbschreibung>,
}

impl Bestandsverzeichnis {
    pub fn is_empty(&self) -> bool {
        self.eintraege.is_empty()
            && self.zuschreibungen.is_empty()
            && self.abschreibungen.is_empty()
    }
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

impl StringOrLines {
    pub fn is_empty(&self) -> bool {
        match self {
            StringOrLines::SingleLine(s) => s.is_empty(),
            StringOrLines::MultiLine(ml) => ml.is_empty(),
        }
    }
}

// Eintrag für ein grundstücksgleiches Recht
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BvEintragRecht {
    pub lfd_nr: usize,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub zu_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bisherige_lfd_nr: Option<usize>,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BvEintragFlurstueck {
    pub lfd_nr: usize,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bisherige_lfd_nr: Option<usize>,
    pub flur: usize,
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub flurstueck: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gemarkung: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bezeichnung: Option<StringOrLines>,
    #[serde(default)]
    #[serde(skip_serializing_if = "FlurstueckGroesse::ist_leer")]
    pub groesse: FlurstueckGroesse,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
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

impl Default for FlurstueckGroesse {
    fn default() -> Self {
        FlurstueckGroesse::Metrisch { m2: None }
    }
}

impl FlurstueckGroesse {
    pub fn ist_leer(&self) -> bool {
        match self {
            FlurstueckGroesse::Metrisch { m2 } => m2.is_none(),
            FlurstueckGroesse::Hektar { ha, a, m2 } => m2.is_none() && ha.is_none() && a.is_none(),
        }
    }
    pub fn get_m2(&self) -> u64 {
        match self {
            FlurstueckGroesse::Metrisch { m2 } => m2.unwrap_or(0),
            FlurstueckGroesse::Hektar { ha, a, m2 } => {
                ha.unwrap_or(0) * 100_000 + a.unwrap_or(0) * 100 + m2.unwrap_or(0)
            }
        }
    }

    pub fn get_ha_string(&self) -> String {
        let m2_string = format!("{}", self.get_m2());
        let mut m2_string_chars: Vec<char> = m2_string.chars().collect();
        for _ in 0..4 {
            m2_string_chars.pop();
        }
        m2_string_chars.iter().collect()
    }

    pub fn get_a_string(&self) -> String {
        let m2_string = format!("{}", self.get_m2());
        let mut m2_string_chars: Vec<char> = m2_string.chars().collect();
        m2_string_chars.reverse();
        for _ in 0..(m2_string_chars.len().saturating_sub(4)) {
            m2_string_chars.pop();
        }
        m2_string_chars.reverse();
        for _ in 0..2 {
            m2_string_chars.pop();
        }
        m2_string_chars.iter().collect()
    }

    pub fn get_m2_string(&self) -> String {
        let m2_string = format!("{}", self.get_m2());
        let mut m2_string_chars: Vec<char> = m2_string.chars().collect();
        m2_string_chars.reverse();
        for _ in 0..(m2_string_chars.len().saturating_sub(2)) {
            m2_string_chars.pop();
        }
        m2_string_chars.reverse();
        let fi: String = m2_string_chars.iter().collect();
        if fi.is_empty() {
            format!("0")
        } else {
            fi
        }
    }
}

impl StringOrLines {
    pub fn text(&self) -> String {
        self.lines().join("\r\n")
    }

    pub fn text_clean(&self) -> String {
        crate::pdf::unhyphenate(&self.lines().join("\r\n"))
    }

    pub fn lines(&self) -> Vec<String> {
        match self {
            StringOrLines::SingleLine(s) => s.lines().map(|s| s.to_string()).collect(),
            StringOrLines::MultiLine(ml) => ml.clone(),
        }
    }
}

impl BvEintrag {
    pub fn ist_geroetet(&self) -> bool {
        match self {
            BvEintrag::Flurstueck(flst) => flst
                .manuell_geroetet
                .unwrap_or(flst.automatisch_geroetet.unwrap_or(false)),
            BvEintrag::Recht(recht) => recht
                .manuell_geroetet
                .unwrap_or(recht.automatisch_geroetet.unwrap_or(false)),
        }
    }
}

impl BvZuschreibung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl BvAbschreibung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt1GrundEintragung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt1EintragV1 {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt1EintragV2 {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt1Eintrag {
    pub fn get_lfd_nr(&self) -> usize {
        match self {
            Abt1Eintrag::V1(v1) => v1.lfd_nr,
            Abt1Eintrag::V2(v2) => v2.lfd_nr,
        }
    }

    pub fn get_eigentuemer(&self) -> StringOrLines {
        match self {
            Abt1Eintrag::V1(v1) => v1.eigentuemer.clone(),
            Abt1Eintrag::V2(v2) => v2.eigentuemer.clone(),
        }
    }

    pub fn ist_geroetet(&self) -> bool {
        match self {
            Abt1Eintrag::V1(v1) => v1.ist_geroetet(),
            Abt1Eintrag::V2(v2) => v2.ist_geroetet(),
        }
    }
}

impl Abt1Veraenderung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt1Loeschung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt2Eintrag {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt2Veraenderung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt2Loeschung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt3Eintrag {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt3Veraenderung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl Abt3Loeschung {
    pub fn ist_geroetet(&self) -> bool {
        self.manuell_geroetet
            .or(self.automatisch_geroetet.clone())
            .unwrap_or(false)
    }
}

impl From<StringOrLines> for String {
    fn from(s: StringOrLines) -> String {
        match s {
            StringOrLines::SingleLine(s) => s,
            StringOrLines::MultiLine(ml) => ml.join("\r\n"),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BvZuschreibung {
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub bv_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BvAbschreibung {
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub bv_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Abteilung1 {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub eintraege: Vec<Abt1Eintrag>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub grundlagen_eintragungen: Vec<Abt1GrundEintragung>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub veraenderungen: Vec<Abt1Veraenderung>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub loeschungen: Vec<Abt1Loeschung>,
}

impl Abteilung1 {
    pub fn is_empty(&self) -> bool {
        self.eintraege.is_empty()
            && self.grundlagen_eintragungen.is_empty()
            && self.veraenderungen.is_empty()
            && self.loeschungen.is_empty()
    }
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
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub eigentuemer: StringOrLines,
    // Used to distinguish from Abt1EintragV1
    pub version: usize,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abt1EintragV1 {
    // lfd. Nr. der Eintragung
    pub lfd_nr: usize,
    // Rechtstext
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub eigentuemer: StringOrLines,
    // lfd. Nr der betroffenen Grundstücke im Bestandsverzeichnis
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub bv_nr: StringOrLines,
    // Vec<BvNr>,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub grundlage_der_eintragung: StringOrLines,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Abt1GrundEintragung {
    // lfd. Nr. der Eintragung
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub bv_nr: StringOrLines,
    // Grundlage der Eintragung
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt1Veraenderung {
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub lfd_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt1Loeschung {
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub lfd_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Abteilung2 {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub eintraege: Vec<Abt2Eintrag>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub veraenderungen: Vec<Abt2Veraenderung>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub loeschungen: Vec<Abt2Loeschung>,
}

impl Abteilung2 {
    pub fn is_empty(&self) -> bool {
        self.eintraege.is_empty() && self.veraenderungen.is_empty() && self.loeschungen.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abt2Eintrag {
    // lfd. Nr. der Eintragung
    pub lfd_nr: usize,
    // lfd. Nr der betroffenen Grundstücke im Bestandsverzeichnis
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub bv_nr: StringOrLines,
    // Rechtstext
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt2Veraenderung {
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub lfd_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt2Loeschung {
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub lfd_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Abteilung3 {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub eintraege: Vec<Abt3Eintrag>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub veraenderungen: Vec<Abt3Veraenderung>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub loeschungen: Vec<Abt3Loeschung>,
}

impl Abteilung3 {
    pub fn is_empty(&self) -> bool {
        self.eintraege.is_empty() && self.veraenderungen.is_empty() && self.loeschungen.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abt3Eintrag {
    // lfd. Nr. der Eintragung
    pub lfd_nr: usize,
    // lfd. Nr der betroffenen Grundstücke im Bestandsverzeichnis
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub bv_nr: StringOrLines,
    // Betrag (EUR / DM)
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub betrag: StringOrLines,
    /// Rechtstext
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt3Veraenderung {
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub lfd_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Abt3Loeschung {
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub lfd_nr: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub betrag: StringOrLines,
    #[serde(default)]
    #[serde(skip_serializing_if = "StringOrLines::is_empty")]
    pub text: StringOrLines,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatisch_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manuell_geroetet: Option<bool>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_in_pdf: Option<PositionInPdf>,
}

#[derive(Debug, Default, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct PositionInPdf {
    pub seite: u32,
    #[serde(default)]
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
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub spalten: BTreeMap<String, Rect>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
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
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub seiten: BTreeMap<String, PdfToTextSeite>,
}

impl PdfToTextLayout {
    pub fn is_empty(&self) -> bool {
        self.seiten.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfToTextSeite {
    pub breite_mm: f32,
    pub hoehe_mm: f32,
    pub texte: Vec<Textblock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenutzerInfo {
    pub id: i32,
    pub rechte: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AbonnementInfo {
    pub amtsgericht: String,
    pub grundbuchbezirk: String,
    pub blatt: i32,
    pub text: String,
    pub aktenzeichen: Option<String>,
}
