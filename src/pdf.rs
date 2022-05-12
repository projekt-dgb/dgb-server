use crate::{Titelblatt, Grundbuch, Abt2Eintrag, Abt3Eintrag, BvEintrag};
use printpdf::{
    BuiltinFont, PdfDocument, Mm, IndirectFontRef,
    PdfDocumentReference, PdfLayerReference,
    Line, Point, Color, Cmyk, Pt,
};
use std::path::Path;
use std::collections::BTreeMap;

pub struct GrundbuchExportConfig {
    pub exportiere: PdfExportTyp,
    pub optionen: GenerateGrundbuchConfig,
}

pub enum PdfExportTyp {
    OffenesGrundbuch(Grundbuch),
    AlleOffenDigitalisiert(Vec<Grundbuch>),
    AlleOffen(Vec<Grundbuch>),
    AlleOriginalPdf(Vec<String>),
}

pub enum GenerateGrundbuchConfig {
    EinzelneDatei {
        datei: String,
        exportiere_bv: bool,
        exportiere_abt1: bool,
        exportiere_abt2: bool,
        exportiere_abt3: bool,
        leere_seite_nach_titelblatt: bool,
        mit_geroeteten_eintraegen: bool,
    },
    MehrereDateien {
        ordner: String,
        exportiere_bv: bool,
        exportiere_abt1: bool,
        exportiere_abt2: bool,
        exportiere_abt3: bool,
        leere_seite_nach_titelblatt: bool,
        mit_geroeteten_eintraegen: bool,
    }
}

impl GenerateGrundbuchConfig {
    pub fn get_options(&self) -> PdfGrundbuchOptions {
        match self {
            GenerateGrundbuchConfig::EinzelneDatei {
                exportiere_bv,
                exportiere_abt1,
                exportiere_abt2,
                exportiere_abt3,
                leere_seite_nach_titelblatt,
                mit_geroeteten_eintraegen,
                ..
            } | GenerateGrundbuchConfig::MehrereDateien {
                exportiere_bv,
                exportiere_abt1,
                exportiere_abt2,
                exportiere_abt3,
                leere_seite_nach_titelblatt,
                mit_geroeteten_eintraegen,
                ..
            } => {
                PdfGrundbuchOptions {
                    exportiere_bv: *exportiere_bv,
                    exportiere_abt1: *exportiere_abt1,
                    exportiere_abt2: *exportiere_abt2,
                    exportiere_abt3: *exportiere_abt3,
                    leere_seite_nach_titelblatt: *leere_seite_nach_titelblatt,
                    mit_geroeteten_eintraegen: *mit_geroeteten_eintraegen,
                }
            }
        }
    }
}

pub struct PdfGrundbuchOptions {
    exportiere_bv: bool,
    exportiere_abt1: bool,
    exportiere_abt2: bool,
    exportiere_abt3: bool,
    leere_seite_nach_titelblatt: bool,
    mit_geroeteten_eintraegen: bool,
}

struct PdfFonts {
    times: IndirectFontRef,
    times_bold: IndirectFontRef,
    courier_bold: IndirectFontRef,
    helvetica: IndirectFontRef,
}

impl PdfFonts {
    fn new(doc: &mut PdfDocumentReference) -> Self {
        Self {
            times_bold: doc.add_builtin_font(BuiltinFont::TimesBoldItalic).unwrap(),
            times: doc.add_builtin_font(BuiltinFont::TimesItalic).unwrap(),
            courier_bold: doc.add_builtin_font(BuiltinFont::CourierBold).unwrap(),
            helvetica: doc.add_builtin_font(BuiltinFont::HelveticaBold).unwrap(),
        }
    }
}

pub fn export_grundbuch(config: GrundbuchExportConfig) -> Result<(), String> {
    match config.optionen {
        GenerateGrundbuchConfig::EinzelneDatei { ref datei, ..  } => {
            export_grundbuch_single_file(config.optionen.get_options(), &config.exportiere, datei.clone())
        },
        GenerateGrundbuchConfig::MehrereDateien { ref ordner, ..  } => {
            export_grundbuch_multi_files(config.optionen.get_options(), &config.exportiere, ordner.clone())
        },
    }
}

fn export_grundbuch_single_file(options: PdfGrundbuchOptions, source: &PdfExportTyp, datei: String) -> Result<(), String> {
    match source {
        PdfExportTyp::AlleOriginalPdf(gb) => {
            let mut files = Vec::new();
            
            for d in gb {
                let bytes = std::fs::read(&d).map_err(|e| format!("Fehler: {}: {}", d, e))?;
                let document = lopdf::Document::load_mem(&bytes).map_err(|e| format!("Fehler: {}: {}", d, e))?;
                files.push(document);
            }
            
            let merged = merge_pdf_files(files).map_err(|e| format!("Fehler: {}: {}", datei, e))?;
            
            let _ = std::fs::write(Path::new(&datei), &merged)
            .map_err(|e| format!("Fehler: {}: {}", datei, e))?;
        },
        PdfExportTyp::OffenesGrundbuch(gb) => {
            
            let grundbuch_von = gb.titelblatt.grundbuch_von.clone();
            let blatt = gb.titelblatt.blatt;
            let amtsgericht = gb.titelblatt.amtsgericht.clone();
    
            let titel = format!("{grundbuch_von} Blatt {blatt} (Amtsgericht {amtsgericht})");
            let (mut doc, page1, layer1) = PdfDocument::new(&titel, Mm(210.0), Mm(297.0), "Titelblatt");
            let titelblatt = format!("{}_{}", gb.titelblatt.grundbuch_von, gb.titelblatt.blatt);
            let fonts = PdfFonts::new(&mut doc);
            
            write_titelblatt(&mut doc.get_page(page1).get_layer(layer1), &fonts, &gb.titelblatt);
            if options.leere_seite_nach_titelblatt {
                // Leere Seite 2
                let (_, _) = doc.add_page(Mm(210.0), Mm(297.0), "Formular");
            }
            write_grundbuch(&mut doc, &gb, &fonts, &options);
            
            let bytes = doc.save_to_bytes().unwrap_or_default();
            let _ = std::fs::write(Path::new(&datei), &bytes) 
            .map_err(|e| format!("Fehler: {}: {}", titelblatt, e))?;
        },
        PdfExportTyp::AlleOffenDigitalisiert(gb) | PdfExportTyp::AlleOffen(gb) => {
           
            let titel = Path::new(&datei).file_name().map(|f| format!("{}", f.to_str().unwrap_or(""))).unwrap_or_default();
            let (mut doc, page1, layer1) = PdfDocument::new(&titel, Mm(210.0), Mm(297.0), "Titelblatt");
            let fonts = PdfFonts::new(&mut doc);
            
            if let Some(f) = gb.get(0) {
                write_titelblatt(&mut doc.get_page(page1).get_layer(layer1), &fonts, &f.titelblatt);
                if options.leere_seite_nach_titelblatt {
                    // Leere Seite 2
                    let (_, _) = doc.add_page(Mm(210.0), Mm(297.0), "Formular");
                }
                write_grundbuch(&mut doc, &f, &fonts, &options);
            }
            
            for f in gb.iter().skip(1) {
                let (titelblatt_page, titelblatt_layer) = doc.add_page(Mm(210.0), Mm(297.0), "Titelblatt");
                write_titelblatt(&mut doc.get_page(titelblatt_page).get_layer(titelblatt_layer), &fonts, &f.titelblatt);
                if options.leere_seite_nach_titelblatt {
                    // Leere Seite 2
                    let (_, _) = doc.add_page(Mm(210.0), Mm(297.0), "Formular");
                }
                write_grundbuch(&mut doc, &f, &fonts, &options);
            }
            
            let bytes = doc.save_to_bytes().unwrap_or_default();
            let _ = std::fs::write(Path::new(&datei), &bytes) 
            .map_err(|e| format!("Fehler: {}: {}", titel, e))?;
        },
    }
    
    Ok(())
}

fn export_grundbuch_multi_files(
    options: PdfGrundbuchOptions, 
    source: &PdfExportTyp, 
    ordner: String
) -> Result<(), String> {
    
    match source {
        PdfExportTyp::AlleOriginalPdf(gb) => {
            for datei in gb {
                let titelblatt = Path::new(&datei).file_name().map(|f| format!("{}", f.to_str().unwrap_or(""))).unwrap_or_default();
                let target_path = Path::new(&ordner).join(&format!("{titelblatt}.pdf"));
                let _ = std::fs::copy(Path::new(&datei), target_path) 
                .map_err(|e| format!("Fehler: {}: {}", titelblatt, e))?;
            }
        },
        PdfExportTyp::OffenesGrundbuch(gb) => {
        
            let grundbuch_von = gb.titelblatt.grundbuch_von.clone();
            let blatt = gb.titelblatt.blatt;
            let amtsgericht = gb.titelblatt.amtsgericht.clone();
            
            let titel = format!("{grundbuch_von} Blatt {blatt} (Amtsgericht {amtsgericht})");
            let (mut doc, page1, layer1) = PdfDocument::new(&titel, Mm(210.0), Mm(297.0), "Titelblatt");
            let titelblatt = format!("{}_{}", gb.titelblatt.grundbuch_von, gb.titelblatt.blatt);
            let target_path = Path::new(&ordner).join(&format!("{titelblatt}.pdf"));
            let fonts = PdfFonts::new(&mut doc);
            
            write_titelblatt(&mut doc.get_page(page1).get_layer(layer1), &fonts, &gb.titelblatt);
            if options.leere_seite_nach_titelblatt {
                // Leere Seite 2
                let (_, _) = doc.add_page(Mm(210.0), Mm(297.0), "Formular");
            }
            write_grundbuch(&mut doc, &gb, &fonts, &options);
            
            let bytes = doc.save_to_bytes().unwrap_or_default();
            let _ = std::fs::write(target_path, &bytes) 
            .map_err(|e| format!("Fehler: {}: {}", titelblatt, e))?;
        },
        PdfExportTyp::AlleOffenDigitalisiert(gb) | PdfExportTyp::AlleOffen(gb) => {
            for f in gb {
                let grundbuch_von = f.titelblatt.grundbuch_von.clone();
                let blatt = f.titelblatt.blatt;
                let amtsgericht = f.titelblatt.amtsgericht.clone();
                let titel = format!("{grundbuch_von} Blatt {blatt} (Amtsgericht {amtsgericht})");
            
                let titelblatt = format!("{}_{}", f.titelblatt.grundbuch_von, f.titelblatt.blatt);
                let target_path = Path::new(&ordner).join(&format!("{titelblatt}.pdf"));
                let (mut doc, page1, layer1) = PdfDocument::new(&titel, Mm(210.0), Mm(297.0), "Titelblatt");
                let target_path = Path::new(&ordner).join(&format!("{titelblatt}.pdf"));
                let fonts = PdfFonts::new(&mut doc);
            
                write_titelblatt(&mut doc.get_page(page1).get_layer(layer1), &fonts, &f.titelblatt);          
                if options.leere_seite_nach_titelblatt {
                    // Leere Seite 2
                    let (_, _) = doc.add_page(Mm(210.0), Mm(297.0), "Formular");
                }
                write_grundbuch(&mut doc, &f, &fonts, &options);
                
                let bytes = doc.save_to_bytes().unwrap_or_default();
                let _ = std::fs::write(target_path, &bytes) 
                .map_err(|e| format!("Fehler: {}: {}", titelblatt, e))?;
            }
        },
    }
    
    Ok(())
}


fn write_titelblatt(
    current_layer: &mut PdfLayerReference, 
    fonts: &PdfFonts,
    titelblatt: &Titelblatt,
) {
    let grundbuch_von = titelblatt.grundbuch_von.clone();
    let blatt =  titelblatt.blatt;
    let amtsgericht = titelblatt.amtsgericht.clone();
    
    let gb = format!("Grundbuch von {grundbuch_von}");
    let blatt_nr = format!("Blatt {blatt}");
    let amtsgericht = format!("Amtsgericht {amtsgericht}");
    
    // text, font size, x from left edge, y from bottom edge, font
    let start = Mm(297.0 / 2.0);
    let rand_x = Mm(25.0);
    current_layer.use_text(&gb, 22.0, Mm(25.0), start, &fonts.times_bold);
    current_layer.add_shape(Line {
        points: vec![
            (Point::new(rand_x, start - Mm(4.5)), false),
            (Point::new(rand_x + Mm(25.0), start - Mm(4.5)), false)
        ],
        is_closed: false,
        has_fill: false,
        has_stroke: true,
        is_clipping_path: false,
    });
        
    current_layer.use_text(&blatt_nr, 16.0, Mm(25.0), start - Mm(12.0), &fonts.times);
    current_layer.use_text(&amtsgericht, 16.0, Mm(25.0), start - Mm(18.0), &fonts.times);
}

fn write_grundbuch(
    doc: &mut PdfDocumentReference, 
    grundbuch: &Grundbuch, 
    fonts: &PdfFonts,
    options: &PdfGrundbuchOptions
) {
    let grundbuch_von = grundbuch.titelblatt.grundbuch_von.clone();
    let blatt =  grundbuch.titelblatt.blatt;
    let amtsgericht = grundbuch.titelblatt.amtsgericht.clone();
    
    let gb = format!("Grundbuch von {grundbuch_von}");
    let blatt_nr = format!("Blatt {blatt}");
    let amtsgericht = format!("Amtsgericht {amtsgericht}");

    let text_rows = get_text_rows(grundbuch, options);
    render_text_rows(doc, fonts, &text_rows);
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfTextRow {
    pub texts: Vec<String>,
    pub header: PdfHeader,
    pub geroetet: Geroetet,
    pub teil_geroetet: BTreeMap<usize, String>,
    pub force_single_line: Vec<usize>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Geroetet {
    Ganz(bool),
    HalbHalb(bool, bool),
}

impl Geroetet {
    fn hat_links_rot(&self) -> bool {
        match self {
            Geroetet::Ganz(a) => *a,
            Geroetet::HalbHalb(a, _) => *a,
        }
    }
    
    fn hat_rechts_rot(&self) -> bool {
        match self {
            Geroetet::Ganz(a) => *a,
            Geroetet::HalbHalb(_, b) => *b,
        }
    }
}

const EXTENT_PER_LINE: f32 = 3.53;
const EXTENT_PER_LINE_X: f32 = 2.2;
const RED: Cmyk = Cmyk { c: 0.0, m: 0.7, y: 0.4, k: 0.0, icc_profile: None };
const BLACK: Cmyk = Cmyk { c: 0.0, m: 0.0, y: 0.0, k: 1.0, icc_profile: None };

impl PdfTextRow {
    
    pub fn get_height_mm(&self) -> f32 {
        self.texts
        .iter()
        .enumerate()
        .map(|(col_id, text)| {
            let max_col_width_for_column = self.header.get_max_col_width(col_id);
            if self.force_single_line.contains(&col_id) {
                EXTENT_PER_LINE
            } else {
                let text_broken_lines = wordbreak_text(&text, max_col_width_for_column);
                text_broken_lines.lines().count() as f32 * EXTENT_PER_LINE
            }
        })
        .map(|s| (s * 1000.0).round() as usize)
        .max()
        .unwrap_or(0) as f32 / 1000.0_f32
    }
    
    fn add_to_page(&self, layer: &mut PdfLayerReference, fonts: &PdfFonts, y_start: f32) {
        
        if self.geroetet.hat_links_rot() {
            layer.set_fill_color(Color::Cmyk(RED));
            layer.set_outline_color(Color::Cmyk(RED));
        }
        
        layer.set_font(&fonts.courier_bold, 10.0);
        layer.set_line_height(10.0);

        let max_width_mm = self.header.get_max_width_mm();
        
        let x_start_mm = self.header.get_starting_x_spalte_mm(0);
        
        let col_id_half = self.texts.len() / 2;
        
        for (col_id, text) in self.texts.iter().enumerate() {
            
            if col_id == col_id_half {
                if self.geroetet.hat_rechts_rot() {
                    layer.set_fill_color(Color::Cmyk(RED));
                    layer.set_outline_color(Color::Cmyk(RED));
                } else {
                    layer.set_fill_color(Color::Cmyk(BLACK));
                    layer.set_outline_color(Color::Cmyk(BLACK));
                }
            }
            
            let max_col_width_for_column = self.header.get_max_col_width(col_id);
            let text_broken_lines = wordbreak_text(&text, max_col_width_for_column);
            layer.begin_text_section();
            layer.set_text_cursor(
                Mm((self.header.get_starting_x_spalte_mm(col_id) + 1.0) as f64),
                Mm(y_start as f64),
            );
            
            if self.force_single_line.contains(&col_id) {
                layer.write_text(text.clone(), &fonts.courier_bold);
            } else {
                for line in text_broken_lines.lines() {
                    layer.write_text(line.clone(), &fonts.courier_bold);
                    layer.add_line_break();
                }
            }
            
            layer.end_text_section();
        }
        
        if self.geroetet.hat_links_rot() || 
           self.geroetet.hat_rechts_rot() {
        
            let self_height = self.get_height_mm();
            
            layer.set_fill_color(Color::Cmyk(RED));
            layer.set_outline_color(Color::Cmyk(RED));
            
            match self.geroetet {
                Geroetet::Ganz(true) | Geroetet::HalbHalb(true, true) => {
                    if self_height == EXTENT_PER_LINE {
                        layer.add_shape(Line {
                            points: vec![
                                (Point::new(Mm(x_start_mm as f64), Mm(y_start as f64)), false),
                                (Point::new(Mm((x_start_mm + max_width_mm) as f64), Mm(y_start as f64)), false),
                            ],
                            is_closed: false,
                            has_fill: false,
                            has_stroke: true,
                            is_clipping_path: false,
                        });
                    } else {
                        layer.add_shape(Line {
                            points: vec![
                                (Point::new(Mm(x_start_mm as f64), Mm((y_start + EXTENT_PER_LINE) as f64)), false),
                                (Point::new(Mm((x_start_mm + max_width_mm) as f64), Mm((y_start + EXTENT_PER_LINE) as f64)), false),
                                (Point::new(Mm(x_start_mm as f64), Mm(((y_start + EXTENT_PER_LINE) - self_height - 1.0) as f64)), false),
                                (Point::new(Mm((x_start_mm + max_width_mm) as f64), Mm(((y_start + EXTENT_PER_LINE) - self_height - 1.0) as f64)), false),
                            ],
                            is_closed: false,
                            has_fill: false,
                            has_stroke: true,
                            is_clipping_path: false,
                        });
                    }
                },
                Geroetet::HalbHalb(true, false) => {
                    if self_height == EXTENT_PER_LINE {
                        layer.add_shape(Line {
                            points: vec![
                                (Point::new(Mm(x_start_mm as f64), Mm(y_start as f64)), false),
                                (Point::new(Mm(x_start_mm  as f64 + (max_width_mm as f64 / 2.0)), Mm(y_start as f64)), false),
                            ],
                            is_closed: false,
                            has_fill: false,
                            has_stroke: true,
                            is_clipping_path: false,
                        });
                    } else {
                        layer.add_shape(Line {
                            points: vec![
                                (Point::new(Mm(x_start_mm as f64), Mm((y_start + EXTENT_PER_LINE) as f64)), false),
                                (Point::new(Mm((x_start_mm  as f64 + (max_width_mm as f64 / 2.0)) as f64), Mm((y_start + EXTENT_PER_LINE) as f64)), false),
                                (Point::new(Mm(x_start_mm as f64), Mm(((y_start + EXTENT_PER_LINE) - self_height - 1.0) as f64)), false),
                                (Point::new(Mm((x_start_mm  as f64 + (max_width_mm as f64 / 2.0)) as f64), Mm(((y_start + EXTENT_PER_LINE) - self_height - 1.0) as f64)), false),
                            ],
                            is_closed: false,
                            has_fill: false,
                            has_stroke: true,
                            is_clipping_path: false,
                        });
                    }
                },
                Geroetet::HalbHalb(false, true) => {
                    if self_height == EXTENT_PER_LINE {
                        layer.add_shape(Line {
                            points: vec![
                                (Point::new(Mm(x_start_mm  as f64 + (max_width_mm as f64 / 2.0) as f64), Mm(y_start as f64)), false),
                                (Point::new(Mm((x_start_mm + max_width_mm) as f64), Mm(y_start as f64)), false),
                            ],
                            is_closed: false,
                            has_fill: false,
                            has_stroke: true,
                            is_clipping_path: false,
                        });
                    } else {
                        layer.add_shape(Line {
                            points: vec![
                                (Point::new(Mm(x_start_mm  as f64 + (max_width_mm as f64 / 2.0) as f64), Mm((y_start + EXTENT_PER_LINE) as f64)), false),
                                (Point::new(Mm((x_start_mm + max_width_mm) as f64), Mm((y_start + EXTENT_PER_LINE) as f64)), false),
                                (Point::new(Mm(x_start_mm  as f64 + (max_width_mm as f64 / 2.0) as f64), Mm(((y_start + EXTENT_PER_LINE) - self_height - 1.0) as f64)), false),
                                (Point::new(Mm((x_start_mm + max_width_mm) as f64), Mm(((y_start + EXTENT_PER_LINE) - self_height - 1.0) as f64)), false),
                            ],
                            is_closed: false,
                            has_fill: false,
                            has_stroke: true,
                            is_clipping_path: false,
                        });
                    }
                },
                _ => { },
            }
            
            layer.set_fill_color(Color::Cmyk(BLACK));
            layer.set_outline_color(Color::Cmyk(BLACK));
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PdfHeader {
    Bestandsverzeichnis,
    BestandsverzeichnisZuAb,
    Abteilung1,
    Abteilung1ZuAb,
    Abteilung2,
    Abteilung2ZuAb,
    Abteilung3,
    Abteilung3ZuAb,
}

fn pt_to_mm(pt: Pt) -> Mm { pt.into() }

impl PdfHeader {

    pub fn get_max_col_width(&self, col_id: usize) -> usize {
        use self::PdfHeader::*;
        let spalten_lines = self.get_spalten_lines();
        let width = ((if col_id == spalten_lines.len() - 1 {
            self.get_starting_x_spalte_mm(0) + 
            self.get_max_width_mm() - 
            self.get_starting_x_spalte_mm(col_id)
        } else {
            self.get_starting_x_spalte_mm(col_id + 1) -
            self.get_starting_x_spalte_mm(col_id)
        } / EXTENT_PER_LINE_X).floor() as usize).max(3);
        width
    }

    pub fn get_starting_x_spalte_mm(&self, spalte_idx: usize) -> f32 {
        self.get_spalten_lines()
        .get(spalte_idx)
        .and_then(|line| Some(pt_to_mm(line.points.get(0)?.0.x).0 as f32))
        .unwrap_or(0.0)
    }
    
    pub fn get_max_width_mm(&self) -> f32 {
        let start_erste_spalte = self.get_starting_x_spalte_mm(0);
        
        let letzte_spalte_x = self.get_spalten_lines()
        .last()
        .map(|last| {
            last.points.iter()
            .map(|(p, _)| { (pt_to_mm(p.x).0 * 1000.0) as usize })
            .max()
            .unwrap_or((start_erste_spalte * 1000.0) as usize) as f32 / 1000.0
        }).unwrap_or(start_erste_spalte);
        
        letzte_spalte_x - start_erste_spalte
    }
    
    pub fn get_start_y(&self) -> f32 {
        self.get_spalten_lines()
        .iter()
        .map(|line| {
            line.points.iter()
            .map(|(p, _)| { (pt_to_mm(p.y).0 * 1000.0) as usize })
            .max()
            .unwrap_or(0)
        })
        .max()
        .unwrap_or(0) as f32 / 1000.0
    }
    
    pub fn get_end_y(&self) -> f32 {
        self.get_spalten_lines()
        .iter()
        .map(|line| {
            line.points.iter()
            .map(|(p, _)| { (pt_to_mm(p.y).0 * 1000.0) as usize })
            .min()
            .unwrap_or(0)
        })
        .min()
        .unwrap_or(0) as f32 / 1000.0
    }
    
    fn get_spalten_lines(&self) -> Vec<Line> {
        
        let mut spalten_lines = Vec::new();
        
        match self {
            PdfHeader::Bestandsverzeichnis => {
                
                let lfd_nr_spalte = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(10.0), Mm(10.0)), false),
                        (Point::new(Mm(25.0), Mm(10.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(lfd_nr_spalte);
                
                let bisherige_lfd_nr_spalte = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(10.0)), false),
                        (Point::new(Mm(40.0), Mm(10.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(bisherige_lfd_nr_spalte);
                
                let gemarkung_spalte = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(10.0)), false),
                        (Point::new(Mm(75.0), Mm(10.0)), false),
                        (Point::new(Mm(75.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(gemarkung_spalte);

                let flur_spalte = Line {
                    points: vec![
                        (Point::new(Mm(75.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(75.0), Mm(10.0)), false),
                        (Point::new(Mm(85.0), Mm(10.0)), false),
                        (Point::new(Mm(85.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(flur_spalte);

                let flurstueck_spalte = Line {
                    points: vec![
                        (Point::new(Mm(85.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(85.0), Mm(10.0)), false),
                        (Point::new(Mm(100.0), Mm(10.0)), false),
                        (Point::new(Mm(100.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(flurstueck_spalte);

                let wirtschaftsart_lage_spalte = Line {
                    points: vec![
                        (Point::new(Mm(100.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(100.0), Mm(10.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(10.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(wirtschaftsart_lage_spalte);
                        
                let ha_spalte = Line {
                    points: vec![
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(10.0)), false),
                        (Point::new(Mm(210.0 - 24.0), Mm(10.0)), false),
                        (Point::new(Mm(210.0 - 24.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(ha_spalte);

                let a_spalte = Line {
                    points: vec![
                        (Point::new(Mm(210.0 - 24.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(210.0 - 24.0), Mm(10.0)), false),
                        (Point::new(Mm(210.0 - 17.0), Mm(10.0)), false),
                        (Point::new(Mm(210.0 - 17.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(a_spalte);
                        
                let m2_spalte = Line {
                    points: vec![
                        (Point::new(Mm(210.0 - 17.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(210.0 - 17.0), Mm(10.0)), false),
                        (Point::new(Mm(210.0 - 10.0), Mm(10.0)), false),
                        (Point::new(Mm(210.0 - 10.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(m2_spalte);
            },    
            PdfHeader::BestandsverzeichnisZuAb | PdfHeader::Abteilung1 |  
            PdfHeader::Abteilung1ZuAb | PdfHeader::Abteilung2ZuAb | 
            PdfHeader::Abteilung3ZuAb => {
                
                let lfd_nr_spalte = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(10.0), Mm(10.0)), false),
                        (Point::new(Mm(25.0), Mm(10.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(lfd_nr_spalte);
                
                let zuschreibungen_spalte = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(10.0)), false),
                        (Point::new(Mm(105.0), Mm(10.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(zuschreibungen_spalte);
                
                let lfd_nr_spalte = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(10.0)), false),
                        (Point::new(Mm(120.0), Mm(10.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(lfd_nr_spalte);
                
                let abschreibungen_spalte = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(10.0)), false),
                        (Point::new(Mm(200.0), Mm(10.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(abschreibungen_spalte);
            },
            PdfHeader::Abteilung2 => {
            
                let lfd_nr_spalte = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(10.0), Mm(10.0)), false),
                        (Point::new(Mm(25.0), Mm(10.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(lfd_nr_spalte);
                
                
                let bv_nr_spalte = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(10.0)), false),
                        (Point::new(Mm(40.0), Mm(10.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(bv_nr_spalte);
                
                
                let text_spalte = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(10.0)), false),
                        (Point::new(Mm(200.0), Mm(10.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(text_spalte);
            },
            PdfHeader::Abteilung3 => {
                
                let lfd_nr_spalte = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(10.0), Mm(10.0)), false),
                        (Point::new(Mm(25.0), Mm(10.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(lfd_nr_spalte);
                
                let bv_nr_spalte = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(10.0)), false),
                        (Point::new(Mm(40.0), Mm(10.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(bv_nr_spalte);
                
                
                let betrag_spalte = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(10.0)), false),
                        (Point::new(Mm(90.0), Mm(10.0)), false),
                        (Point::new(Mm(90.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(betrag_spalte);

                
                let text_spalte = Line {
                    points: vec![
                        (Point::new(Mm(90.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(90.0), Mm(10.0)), false),
                        (Point::new(Mm(200.0), Mm(10.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                spalten_lines.push(text_spalte);
            },
        }
        
        spalten_lines
    }
    
    fn add_to_page(&self, layer: &mut PdfLayerReference, fonts: &PdfFonts) {
        layer.save_graphics_state();
       
        layer.use_text(
            match self {
                PdfHeader::Bestandsverzeichnis => "Bestandsverzeichnis",
                PdfHeader::BestandsverzeichnisZuAb => "Bestandsverzeichnis - Veränderungen",
                PdfHeader::Abteilung1 => "Abteilung 1",
                PdfHeader::Abteilung1ZuAb => "Abteilung 1 - Veränderungen",
                PdfHeader::Abteilung2 => "Abteilung 2",
                PdfHeader::Abteilung2ZuAb => "Abteilung 2 - Veränderungen",
                PdfHeader::Abteilung3 => "Abteilung 3",
                PdfHeader::Abteilung3ZuAb => "Abteilung 3 - Veränderungen",
            }, 
            16.0, 
            Mm(10.0), 
            Mm(297.0 - 16.0), 
            &fonts.times_bold
        );
        
        layer.set_outline_thickness(1.3);

        let border = Line {
            points: vec![
                (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                (Point::new(Mm(10.0), Mm(10.0)), false),
                (Point::new(Mm(210.0 - 10.0), Mm(10.0)), false),
                (Point::new(Mm(210.0 - 10.0), Mm(297.0 - 18.5)), false)
            ],
            is_closed: true,
            has_fill: false,
            has_stroke: true,
            is_clipping_path: false,
        };
        
        layer.add_shape(border);
        
        layer.set_outline_thickness(0.75);

        match self {
            PdfHeader::Bestandsverzeichnis => {
                
                let text_1 = &[
                    ("Laufende",    13.0_f64, 297.0_f64 - 21.0), 
                    ("Nummer",      14.0,     297.0 - 23.5),
                    ("der",         15.5,     297.0 - 26.0), 
                    ("Grund-",      14.0,     297.0 - 28.5), 
                    ("stücke",      14.0,     297.0 - 31.0),
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("1",    17.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_2 = &[
                    ("Bisherige",       28.0_f64, 297.0_f64 - 21.0), 
                    ("laufende",        28.5,     297.0 - 23.5),
                    ("Nummer",          28.5,     297.0 - 26.0), 
                    ("der Grund-",      27.5,     297.0 - 28.5), 
                    ("stücke",          29.5,     297.0 - 31.0),
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_2 = &[
                    ("2",    32.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_3 = &[(
                    "Bezeichnung der Grundstücke und der mit dem Eigentum verbundenen Rechte",       
                    60.0_f64, 297.0_f64 - 21.0
                )];
                
                let text_3_header = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 22.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 22.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_3_header);
                
                for (t, x, y) in text_3.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_3 = &[
                    ("3",    100.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_3_header = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_3_header);
                
                for (t, x, y) in text_3.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_4 = &[(
                    "Größe",       
                    (210.0 - 28.0) as f64, 297.0_f64 - 21.0
                )];
                
                let text_4_header = Line {
                    points: vec![
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 22.0)), false),
                        (Point::new(Mm(210.0 - 10.0), Mm(297.0 - 22.0)), false),
                        (Point::new(Mm(210.0 - 10.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_4_header);
                
                for (t, x, y) in text_4.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_4 = &[
                    ("4",    (210.0 - 25.5) as f64, 297.0_f64 - 34.5)
                ];
                
                let text_4_header = Line {
                    points: vec![
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(210.0 - 10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(210.0 - 10.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_4_header);
                
                for (t, x, y) in text_4.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_4_header = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 22.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 22.0)), false),
                        (Point::new(Mm(210.0 - 40.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_4_header);
                
                let text_4 = &[
                    ("Gemarkung*",              50.0_f64, 297.0_f64 - 31.0),
                    ("Flur",                    77.5_f64, 297.0_f64 - 31.0),
                    ("Flurstück",               87.5_f64, 297.0_f64 - 31.0),
                    ("Wirtschaftsart und Lage", 120.5_f64, 297.0_f64 - 31.0),
                    ("* Wenn die Angabe der Gemarkung fehlt, stimmt ihre Bezeichnung mit der des Grundbuchbezirks überein.", 10.0_f64, 7_f64),
                ];
                
                for (t, x, y) in text_4.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_4 = &[
                    ("ha", (210.0 - 32.5) as f64, 297.0_f64 - 31.0),
                    ("a",  (210.0 - 21.0) as f64, 297.0_f64 - 31.0),
                    ("m²", (210.0 - 15.0) as f64, 297.0_f64 - 31.0),
                ];
                
                for (t, x, y) in text_4.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
            },
            PdfHeader::BestandsverzeichnisZuAb => {
                
                let text_1 = &[
                    ("Bestand und Zuschreibungen",    50.0_f64, 297.0_f64 - 20.7), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Zur lfd.",    14.0_f64, 297.0_f64 - 23.7), 
                    ("Nr. der",     14.0,     297.0 - 26.0),
                    ("Grund-",      13.7,     297.0 - 28.5), 
                    ("stücke",      14.0,     297.0 - 31.0), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("5",    17.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("6",    60.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                // ----
                
                
                let text_1 = &[
                    ("Abschreibungen",    150.0_f64, 297.0_f64 - 20.7), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Zur lfd.",    109.0_f64, 297.0_f64 - 23.7), 
                    ("Nr. der",     109.0,     297.0 - 26.0),
                    ("Grund-",      108.7,     297.0 - 28.5), 
                    ("stücke",      109.0,     297.0 - 31.0), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("7",    112.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("8",    155.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
            },
            PdfHeader::Abteilung1 => {
                                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                
                let text_1 = &[
                    ("Lfd. Nr.",     13.5_f64, 297.0_f64 - 22.5), 
                    ("der",         15.5,     297.0 - 25.0),
                    ("Eintra-",     14.0,     297.0 - 27.5), 
                    ("gungen",      13.5,     297.0 - 30.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("1",    17.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Eigentümer",     55.0_f64, 297.0_f64 - 26.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("2",    60.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                // ----
                                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                
                let text_1 = &[
                    ("Lfd. Nr. der",    107.7_f64, 297.0_f64 - 21.7), 
                    ("Grundstücke",     106.7,     297.0 - 24.0),
                    ("im Bestands-",    107.4,     297.0 - 26.5), 
                    ("verzeichnis",     107.7,     297.0 - 29.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 5.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("3",    112.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Grundlage der Eintragungen",     147.5_f64, 297.0_f64 - 26.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("4",    160.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
            },
            PdfHeader::Abteilung1ZuAb => {
            
                let text_1 = &[
                    ("Veränderungen",    60.0_f64, 297.0_f64 - 20.7), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Zur lfd.",    14.0_f64, 297.0_f64 - 23.7), 
                    ("Nr. der",     14.0,     297.0 - 26.0),
                    ("Grund-",      13.7,     297.0 - 28.5), 
                    ("stücke",      14.0,     297.0 - 31.0), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("5",    17.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("6",    60.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                // ----
                
                
                let text_1 = &[
                    ("Löschungen",    155.0_f64, 297.0_f64 - 20.7), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Zur lfd.",    109.0_f64, 297.0_f64 - 23.7), 
                    ("Nr. der",     109.0,     297.0 - 26.0),
                    ("Grund-",      108.7,     297.0 - 28.5), 
                    ("stücke",      109.0,     297.0 - 31.0), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("7",    112.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("8",    155.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
            },
            PdfHeader::Abteilung2 => {
                     
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                
                let text_1 = &[
                    ("Lfd. Nr.",     13.5_f64, 297.0_f64 - 22.5), 
                    ("der",         15.5,     297.0 - 25.0),
                    ("Eintra-",     14.0,     297.0 - 27.5), 
                    ("gungen",      13.5,     297.0 - 30.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_1 = &[
                    ("1",    17.0_f64, 297.0_f64 - 34.5)
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("2",    32.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Lfd. Nr. der",    27.7_f64, 297.0_f64 - 20.7), 
                    ("betroffenen",     27.2,     297.0 - 23.0),
                    ("Grundstücke",     26.7,     297.0 - 25.5),
                    ("im Bestands-",    27.2,     297.0 - 28.0), 
                    ("verzeichnis",     27.7,     297.0 - 30.5), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 5.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Lasten und Beschränkungen",     105.5_f64, 297.0_f64 - 26.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_2 = &[
                    ("3",    120.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
            },
            PdfHeader::Abteilung2ZuAb => {
                
                let text_1 = &[
                    ("Veränderungen",    50.0_f64, 297.0_f64 - 20.7), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Zur lfd.",    14.0_f64, 297.0_f64 - 23.7), 
                    ("Nr. der",     14.0,     297.0 - 26.0),
                    ("Grund-",      13.7,     297.0 - 28.5), 
                    ("stücke",      14.0,     297.0 - 31.0), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("4",    17.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("5",    60.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                // ----
                
                
                let text_1 = &[
                    ("Löschungen",    145.0_f64, 297.0_f64 - 20.7), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Zur lfd.",    109.0_f64, 297.0_f64 - 23.7), 
                    ("Nr. der",     109.0,     297.0 - 26.0),
                    ("Grund-",      108.7,     297.0 - 28.5), 
                    ("stücke",      109.0,     297.0 - 31.0), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("6",    112.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("7",    155.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
            },
            PdfHeader::Abteilung3 => {
            
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                
                let text_1 = &[
                    ("Lfd. Nr.",     13.5_f64, 297.0_f64 - 22.5), 
                    ("der",         15.5,     297.0 - 25.0),
                    ("Eintra-",     14.0,     297.0 - 27.5), 
                    ("gungen",      13.5,     297.0 - 30.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_1 = &[
                    ("1",    17.0_f64, 297.0_f64 - 34.5)
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("2",    32.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Lfd. Nr. der",    27.7_f64, 297.0_f64 - 20.7), 
                    ("betroffenen",     27.2,     297.0 - 23.0),
                    ("Grundstücke",     26.7,     297.0 - 25.5),
                    ("im Bestands-",    27.2,     297.0 - 28.0), 
                    ("verzeichnis",     27.7,     297.0 - 30.5), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 5.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Betrag",     60.5_f64, 297.0_f64 - 26.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(90.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(90.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_1 = &[
                    ("Hypotheken, Grundschulden, Rentenschulden",     125.5_f64, 297.0_f64 - 26.0), 
                ];
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(90.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(90.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(40.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(40.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(90.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(90.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                let text_2 = &[
                    ("3",    62.0_f64, 297.0_f64 - 34.5)
                ];
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(90.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(90.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                let text_2 = &[
                    ("4",    135.0_f64, 297.0_f64 - 34.5)
                ];
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
            },
            PdfHeader::Abteilung3ZuAb => {
                
                let text_1 = &[
                    ("Veränderungen",    50.0_f64, 297.0_f64 - 20.7), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Zur lfd.",    14.0_f64, 297.0_f64 - 23.7), 
                    ("Nr. der",     14.0,     297.0 - 26.0),
                    ("Grund-",      13.7,     297.0 - 28.5), 
                    ("stücke",      14.0,     297.0 - 31.0), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("5",    17.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("6",    60.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                // ----
                
                
                let text_1 = &[
                    ("Löschungen",    145.0_f64, 297.0_f64 - 20.7), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 18.5)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 18.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("Zur lfd.",    109.0_f64, 297.0_f64 - 23.7), 
                    ("Nr. der",     109.0,     297.0 - 26.0),
                    ("Grund-",      108.7,     297.0 - 28.5), 
                    ("stücke",      109.0,     297.0 - 31.0), 
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(10.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(10.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(25.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                let text_1 = &[
                    ("7",    112.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(105.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(105.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                for (t, x, y) in text_1.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
                
                
                let text_1_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 21.5)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 21.5)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_1_header);
                
                let text_2 = &[
                    ("8",    155.0_f64, 297.0_f64 - 34.5)
                ];
                
                let text_2_header = Line {
                    points: vec![
                        (Point::new(Mm(120.0), Mm(297.0 - 32.0)), false),
                        (Point::new(Mm(120.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 36.0)), false),
                        (Point::new(Mm(200.0), Mm(297.0 - 32.0)), false)
                    ],
                    is_closed: true,
                    has_fill: false,
                    has_stroke: true,
                    is_clipping_path: false,
                };
                
                layer.add_shape(text_2_header);
                
                for (t, x, y) in text_2.iter() {
                    layer.use_text(*t, 6.0, Mm(*x), Mm(*y), &fonts.helvetica);        
                }
            },
        }
        
        layer.restore_graphics_state();
    }
    
    
    pub fn add_columns_to_page(&self, layer: &mut PdfLayerReference) {
        
        layer.save_graphics_state();
        layer.set_outline_thickness(0.5);

        for l in self.get_spalten_lines() {
            layer.add_shape(l);
        }
        
        layer.restore_graphics_state();
    }
}

fn get_text_rows(grundbuch: &Grundbuch, options: &PdfGrundbuchOptions) -> Vec<PdfTextRow> {
    
    let mut rows = Vec::new();
    let mit_geroeteten_eintraegen = options.mit_geroeteten_eintraegen;
    let grundbuch_von = grundbuch.titelblatt.grundbuch_von.clone();
    
    if options.exportiere_bv {
        
        for bv in grundbuch.bestandsverzeichnis.eintraege.iter() {
            match bv {
                BvEintrag::Flurstueck(flst) => {
                    let ha_string = flst.groesse.get_ha_string();
                    let pad_ha_string = " ".repeat(6_usize.saturating_sub(ha_string.trim().len()));
                    
                    rows.push(PdfTextRow {
                        texts: vec![
                            format!("{}", flst.lfd_nr),
                            flst.bisherige_lfd_nr.clone().map(|b| format!("{}", b)).unwrap_or_default(),
                            flst.gemarkung.clone().map(|g| if g == grundbuch_von { String::new() } else { g }).unwrap_or_default(),
                            format!("{}", flst.flur),
                            format!("{}", flst.flurstueck),
                            flst.bezeichnung.clone().unwrap_or_default().text(),
                            format!("{pad_ha_string}{ha_string}"),
                            flst.groesse.get_a_string(),
                            flst.groesse.get_m2_string(),
                        ],
                        header: PdfHeader::Bestandsverzeichnis,
                        geroetet: Geroetet::Ganz(bv.ist_geroetet()),
                        teil_geroetet: BTreeMap::new(),
                        force_single_line: vec![6]
                    });
                },
                BvEintrag::Recht(hvm) => {
                }
            }
        }
        
        let zu_ab_len = grundbuch.bestandsverzeichnis.zuschreibungen.len().max(grundbuch.bestandsverzeichnis.abschreibungen.len());
        
        for i in 0..zu_ab_len {
            
            rows.push(PdfTextRow {
                texts: vec![
                    grundbuch.bestandsverzeichnis.zuschreibungen.get(i).map(|bvz| bvz.bv_nr.text()).unwrap_or_default(),
                    grundbuch.bestandsverzeichnis.zuschreibungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                    grundbuch.bestandsverzeichnis.abschreibungen.get(i).map(|bvz| bvz.bv_nr.text()).unwrap_or_default(),
                    grundbuch.bestandsverzeichnis.abschreibungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::BestandsverzeichnisZuAb,
                geroetet: Geroetet::HalbHalb(
                    grundbuch.bestandsverzeichnis.zuschreibungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    grundbuch.bestandsverzeichnis.abschreibungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
            });
        }
    }
    
    if options.exportiere_abt1 {

        let zu_ab_len = grundbuch.abt1.eintraege.len().max(grundbuch.abt1.grundlagen_eintragungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    grundbuch.abt1.eintraege.get(i).map(|bvz| format!("{}", bvz.get_lfd_nr())).unwrap_or_default(),
                    grundbuch.abt1.eintraege.get(i).map(|bvz| bvz.get_eigentuemer()).unwrap_or_default(),
                    grundbuch.abt1.grundlagen_eintragungen.get(i).map(|bvz| bvz.bv_nr.text()).unwrap_or_default(),
                    grundbuch.abt1.grundlagen_eintragungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::Abteilung1,
                geroetet: Geroetet::HalbHalb(
                    grundbuch.abt1.eintraege.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    grundbuch.abt1.grundlagen_eintragungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
            });
        }
        
        let zu_ab_len = grundbuch.abt1.veraenderungen.len().max(grundbuch.abt1.loeschungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    grundbuch.abt1.veraenderungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    grundbuch.abt1.veraenderungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                    grundbuch.abt1.loeschungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    grundbuch.abt1.loeschungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::Abteilung1ZuAb,
                geroetet: Geroetet::HalbHalb(
                    grundbuch.abt1.veraenderungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    grundbuch.abt1.loeschungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
            });
        }
    }
    
    if options.exportiere_abt2 {
        for abt2 in grundbuch.abt2.eintraege.iter() {
            rows.push(PdfTextRow {
                texts: vec![
                    format!("{}", abt2.lfd_nr),
                    abt2.bv_nr.text(),
                    abt2.text.text(),
                ],
                header: PdfHeader::Abteilung2,
                geroetet: Geroetet::Ganz(abt2.ist_geroetet()),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new()
            });
        }
        
        let zu_ab_len = grundbuch.abt2.veraenderungen.len().max(grundbuch.abt2.loeschungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    grundbuch.abt2.veraenderungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    grundbuch.abt2.veraenderungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                    grundbuch.abt2.loeschungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    grundbuch.abt2.loeschungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::Abteilung2ZuAb,
                geroetet: Geroetet::HalbHalb(
                    grundbuch.abt2.veraenderungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    grundbuch.abt2.loeschungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
            });
        }
    }
    
    if options.exportiere_abt3 {
        for abt3 in grundbuch.abt3.eintraege.iter() {
            rows.push(PdfTextRow {
                texts: vec![
                    format!("{}", abt3.lfd_nr),
                    abt3.bv_nr.text(),
                    abt3.betrag.text(),
                    abt3.text.text(),
                ],
                header: PdfHeader::Abteilung3,
                geroetet: Geroetet::Ganz(abt3.ist_geroetet()),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new()
            });
        }
        
        let zu_ab_len = grundbuch.abt3.veraenderungen.len().max(grundbuch.abt3.loeschungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    grundbuch.abt3.veraenderungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    grundbuch.abt3.veraenderungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                    grundbuch.abt3.loeschungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    grundbuch.abt3.loeschungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::Abteilung3ZuAb,
                geroetet: Geroetet::HalbHalb(
                    grundbuch.abt3.veraenderungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    grundbuch.abt3.loeschungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
            });
        }
    }
    
    rows
}

fn render_text_rows(doc: &mut PdfDocumentReference, fonts: &PdfFonts, blocks: &[PdfTextRow]) {
    
    if blocks.is_empty() { 
        return; 
    }    
    
    let (mut page, mut layer) = doc.add_page(Mm(210.0), Mm(297.0), "Formular");
    let  current_block = &blocks[0];
    current_block.header.add_to_page(&mut doc.get_page(page).get_layer(layer), fonts);
    current_block.header.add_columns_to_page(&mut doc.get_page(page).get_layer(layer));
    
    let mut current_y = current_block.header.get_start_y() - EXTENT_PER_LINE - 0.5;
    current_block.add_to_page(&mut doc.get_page(page).get_layer(layer), fonts, current_y);
    current_y -= current_block.get_height_mm() + 2.0;
    let mut current_header = current_block.header;
    
    for b in blocks.iter().skip(1) {
        
        if b.header != current_header || current_y - b.get_height_mm() - 2.0 < current_header.get_end_y() {
            current_header = b.header;
            current_y = b.header.get_start_y() - EXTENT_PER_LINE - 0.5;
            let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "Formular");
            b.header.add_to_page(&mut doc.get_page(new_page).get_layer(new_layer), fonts);
            b.header.add_columns_to_page(&mut doc.get_page(new_page).get_layer(new_layer));
            page = new_page;
            layer = new_layer;
        }
        
        b.add_to_page(&mut doc.get_page(page).get_layer(layer), fonts, current_y);
        current_y -= b.get_height_mm() + 2.0;
    }
}

// https://www.dariocancelliere.it/blog/2020/09/29/pdf-manipulation-with-rust-and-considerations
fn merge_pdf_files(documents: Vec<lopdf::Document>) -> Result<Vec<u8>, String> {
    
    use lopdf::{Document, Object, ObjectId};
    use std::io::BufWriter;
    
    // Define a starting max_id (will be used as start index for object_ids)
    let mut max_id = 1;

    // Collect all Documents Objects grouped by a map
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();

    for mut document in documents {
        document.renumber_objects_with(max_id);

        max_id = document.max_id + 1;

        documents_pages.extend(
            document
                    .get_pages()
                    .into_iter()
                    .map(|(_, object_id)| {
                        (
                            object_id,
                            document.get_object(object_id).unwrap().to_owned(),
                        )
                    })
                    .collect::<BTreeMap<ObjectId, Object>>(),
        );
        documents_objects.extend(document.objects);
    }

    // Initialize a new empty document
    let mut document = Document::with_version("1.5");

    // Catalog and Pages are mandatory
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    // Process all objects except "Page" type
    for (object_id, object) in documents_objects.iter() {
        // We have to ignore "Page" (as are processed later), "Outlines" and "Outline" objects
        // All other objects should be collected and inserted into the main Document
        match object.type_name().unwrap_or("") {
            "Catalog" => {
                // Collect a first "Catalog" object and use it for the future "Pages"
                catalog_object = Some((
                    if let Some((id, _)) = catalog_object {
                        id
                    } else {
                        *object_id
                    },
                    object.clone(),
                ));
            }
            "Pages" => {
                // Collect and update a first "Pages" object and use it for the future "Catalog"
                // We have also to merge all dictionaries of the old and the new "Pages" object
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref object)) = pages_object {
                        if let Ok(old_dictionary) = object.as_dict() {
                            dictionary.extend(old_dictionary);
                        }
                    }

                    pages_object = Some((
                        if let Some((id, _)) = pages_object {
                            id
                        } else {
                            *object_id
                        },
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            "Page" => {}     // Ignored, processed later and separately
            "Outlines" => {} // Ignored, not supported yet
            "Outline" => {}  // Ignored, not supported yet
            _ => {
                document.objects.insert(*object_id, object.clone());
            }
        }
    }

    // If no "Pages" found abort
    if pages_object.is_none() {
        return Err(format!("Pages root not found."));
    }

    // Iter over all "Page" and collect with the parent "Pages" created before
    for (object_id, object) in documents_pages.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_object.as_ref().unwrap().0);

            document
                    .objects
                    .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    // If no "Catalog" found abort
    if catalog_object.is_none() {
        return Err(format!("Catalog root not found."));
    }

    let catalog_object = catalog_object.unwrap();
    let pages_object = pages_object.unwrap();

    // Build a new "Pages" with updated fields
    if let Ok(dictionary) = pages_object.1.as_dict() {
        let mut dictionary = dictionary.clone();

        // Set new pages count
        dictionary.set("Count", documents_pages.len() as u32);

        // Set new "Kids" list (collected from documents pages) for "Pages"
        dictionary.set(
            "Kids",
            documents_pages
                    .into_iter()
                    .map(|(object_id, _)| Object::Reference(object_id))
                    .collect::<Vec<_>>(),
        );

        document
                .objects
                .insert(pages_object.0, Object::Dictionary(dictionary));
    }

    // Build a new "Catalog" with updated fields
    if let Ok(dictionary) = catalog_object.1.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_object.0);
        dictionary.remove(b"Outlines"); // Outlines not supported in merged PDFs

        document
                .objects
                .insert(catalog_object.0, Object::Dictionary(dictionary));
    }

    document.trailer.set("Root", catalog_object.0);

    // Update the max internal ID as wasn't updated before due to direct objects insertion
    document.max_id = document.objects.len() as u32;

    // Reorder all new Document objects
    document.renumber_objects();
    document.compress();

    // Save the merged PDF
    let mut bytes = Vec::new();
    let mut writer = BufWriter::new(&mut bytes);
    document.save_to(&mut writer)
    .map_err(|e| format!("{}", e))?;
    std::mem::drop(writer);
    Ok(bytes)
}

// Format a string so that it fits into N characters per line
fn wordbreak_text(s: &str, max_cols: usize) -> String {
    
    let mut lines = s.lines()
    .map(|l| l.split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>())
    .collect::<Vec<_>>();
    
    let mut output = String::new();
    
    for words in lines {
        let mut line_len = 0;

        for w in words {
            
            let word_len = w.chars().count() + 1;
            let (before, after) = split_hyphenate(&w, max_cols.saturating_sub(line_len).saturating_sub(1));
            
            if !before.is_empty() {
               if !after.is_empty() {
                    output.push_str(&before);
                    output.push_str("-\r\n");
                    output.push_str(&after);
                    output.push_str(" ");
                    line_len = after.chars().count() + 1;
                } else {
                    output.push_str(&before);
                    output.push_str(" ");
                    line_len += before.chars().count() + 1;
                }
            } else if !after.is_empty() {
                output.push_str("\r\n");
                output.push_str(&after);
                output.push_str(" ");
                line_len = after.chars().count() + 1;
            }
        }
        
        output.push_str("\r\n");
    }
        
    output.trim().to_string()
}

fn split_hyphenate(word: &str, remaining: usize) -> (String, String) {
    
    if remaining == 0 {
        return (String::new(), word.to_string());
    }
    
    let mut before = String::new();
    let mut after = String::new();
    let mut counter = 0;
    
    for syllable in get_syllables(word) {
        let syllable_len = syllable.chars().count();
        if counter + syllable_len > remaining {
            after.push_str(&syllable);
        } else {
            before.push_str(&syllable);
        }
        counter += syllable_len;
    }
    
    (before, after)
}

fn get_syllables(s: &str) -> Vec<String> {
    let vocals = ['a', 'e', 'i', 'o', 'u', 'ö', 'ä', 'ü', 'y'];
    let vocals2 = ['a', 'e', 'i', 'o', 'u', 'y'];

    let mut results = Vec::new();
    let chars = s.chars().collect::<Vec<_>>();
    let mut current_position = chars.len() - 1;
    let mut last_split = 0;
    for i in 0..current_position {
        if i != 0 && 
            vocals.contains(&chars[i]) && 
            !vocals2.contains(&chars[i - 1]) && 
            i - last_split > 1 {
            let a = &chars[last_split..i];
            let b = &chars[i..];
            last_split = i;
            results.push(a.iter().collect::<String>());
        }
    }
    
    results.push((&chars[last_split..]).iter().collect::<String>());
    
    results
}
