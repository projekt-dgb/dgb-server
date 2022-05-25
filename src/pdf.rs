use crate::models::{Titelblatt, BvEintrag, Grundbuch, Abt2Eintrag, Abt3Eintrag};
use printpdf::{
    BuiltinFont, PdfDocument, Mm, IndirectFontRef,
    PdfDocumentReference, PdfLayerReference,
    Line, Point, Color, Cmyk, Pt,
};
use std::path::Path;
use std::collections::BTreeMap;
use hyphenation::{Language, Load, Standard};
use textwrap::{Options, WordSplitter};
use regex::Regex;

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
    pub exportiere_bv: bool,
    pub exportiere_abt1: bool,
    pub exportiere_abt2: bool,
    pub exportiere_abt3: bool,
    pub leere_seite_nach_titelblatt: bool,
    pub mit_geroeteten_eintraegen: bool,
}

pub struct PdfFonts {
    times: IndirectFontRef,
    times_bold: IndirectFontRef,
    courier_bold: IndirectFontRef,
    helvetica: IndirectFontRef,
}

impl PdfFonts {
    pub fn new(doc: &mut PdfDocumentReference) -> Self {
        Self {
            times_bold: doc.add_builtin_font(BuiltinFont::TimesBoldItalic).unwrap(),
            times: doc.add_builtin_font(BuiltinFont::TimesItalic).unwrap(),
            courier_bold: doc.add_builtin_font(BuiltinFont::CourierBold).unwrap(),
            helvetica: doc.add_builtin_font(BuiltinFont::HelveticaBold).unwrap(),
        }
    }
}

pub fn write_titelblatt(
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

pub fn write_grundbuch(
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
    pub hvm_exception: Option<String>,
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
const WHITE: Cmyk = Cmyk { c: 0.0, m: 0.0, y: 0.0, k: 0.0, icc_profile: None };

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
        + if self.hvm_exception.is_some() { 10.0 } else { 0.0 }
    }
    
    fn add_to_page(&self, layer: &mut PdfLayerReference, fonts: &PdfFonts, y_start: f32) {
        
        if self.geroetet.hat_links_rot() {
            layer.set_fill_color(Color::Cmyk(RED));
            layer.set_outline_color(Color::Cmyk(RED));
        }

        let max_width_mm = self.header.get_max_width_mm();
        let self_height = self.get_height_mm();
        let x_start_mm = self.header.get_starting_x_spalte_mm(0);
        let col_id_half = self.texts.len() / 2;
        
        if let Some(s) = self.hvm_exception.as_ref() {
        
            layer.save_graphics_state();
            layer.set_outline_thickness(0.75);
            layer.set_fill_color(Color::Cmyk(WHITE));
            layer.add_shape(Line {
                points: vec![
                    (Point::new(Mm(x_start_mm as f64), Mm(y_start as f64)), false),
                    (Point::new(Mm(x_start_mm as f64), Mm(y_start as f64 - self_height as f64)), false),
                    (Point::new(Mm(x_start_mm as f64 + max_width_mm as f64), Mm(y_start as f64 - self_height as f64)), false),
                    (Point::new(Mm(x_start_mm as f64 + max_width_mm as f64), Mm(y_start as f64)), false)
                ],
                is_closed: true,
                has_fill: true,
                has_stroke: true,
                is_clipping_path: false,
            });
            layer.restore_graphics_state();
            
            layer.begin_text_section();
            layer.set_font(&fonts.helvetica, 6.0);
            layer.set_line_height(6.0);
            layer.set_text_cursor(
                Mm((self.header.get_starting_x_spalte_mm(0) + 1.0) as f64),
                Mm(y_start as f64 - 3.5),
            );
            layer.end_text_section();
            layer.write_text(&format!("HERRSCHVERMERK ZU LFD. NR. {}", unhyphenate(&s)), &fonts.helvetica);
        }
        
        layer.set_font(&fonts.courier_bold, 10.0);
        layer.set_line_height(10.0);
        
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
            
            let max_col_width_for_column = if self.hvm_exception.is_some() && col_id == 2 {
                80
            } else {
                self.header.get_max_col_width(col_id)            
            };
            
            let text_broken_lines = wordbreak_text(&text, max_col_width_for_column);
            layer.begin_text_section();
            layer.set_text_cursor(
                Mm((self.header.get_starting_x_spalte_mm(col_id) + 1.0) as f64),
                Mm(y_start as f64 - if self.hvm_exception.is_some() { 7.5 } else { 0.0 }),
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

fn get_text_rows(
    grundbuch: &Grundbuch, 
    options: &PdfGrundbuchOptions
) -> Vec<PdfTextRow> {
    
    let mut rows = Vec::new();
    let mit_geroeteten_eintraegen = options.mit_geroeteten_eintraegen;
    let grundbuch_von = grundbuch.titelblatt.grundbuch_von.clone();
    
    if options.exportiere_bv {
        
        for bv in grundbuch.bestandsverzeichnis.eintraege.iter() {
            if !mit_geroeteten_eintraegen && bv.ist_geroetet() { continue; }
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
                        force_single_line: vec![6],
                        hvm_exception: None,
                    });
                },
                BvEintrag::Recht(hvm) => {
                    rows.push(PdfTextRow {
                        texts: vec![
                            format!("{}", hvm.lfd_nr),
                            hvm.bisherige_lfd_nr.clone().map(|b| format!("{}", b)).unwrap_or_default(),
                            hvm.text.clone().into(),
                        ],
                        header: PdfHeader::Bestandsverzeichnis,
                        geroetet: Geroetet::Ganz(bv.ist_geroetet()),
                        teil_geroetet: BTreeMap::new(),
                        force_single_line: vec![6],
                        hvm_exception: Some(hvm.zu_nr.clone().text()),
                    });
                }
            }
        }
        
        let zuschreibungen = if !mit_geroeteten_eintraegen { 
            grundbuch.bestandsverzeichnis.zuschreibungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.bestandsverzeichnis.zuschreibungen.clone()
        };
        
        let abschreibungen = if !mit_geroeteten_eintraegen {
            grundbuch.bestandsverzeichnis.abschreibungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.bestandsverzeichnis.abschreibungen.clone()
        };
        
        let zu_ab_len = zuschreibungen.len().max(abschreibungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    zuschreibungen.get(i).map(|bvz| bvz.bv_nr.text()).unwrap_or_default(),
                    zuschreibungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                    abschreibungen.get(i).map(|bvz| bvz.bv_nr.text()).unwrap_or_default(),
                    abschreibungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::BestandsverzeichnisZuAb,
                geroetet: Geroetet::HalbHalb(
                    zuschreibungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    abschreibungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
                hvm_exception: None,
            });
        }
    }
    
    if options.exportiere_abt1 {

        let eintraege = if !mit_geroeteten_eintraegen {
            grundbuch.abt1.eintraege
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.abt1.eintraege.clone()
        };
        
        let grundlagen_eintragungen = if !mit_geroeteten_eintraegen {
            grundbuch.abt1.grundlagen_eintragungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.abt1.grundlagen_eintragungen.clone()
        };
        
        let zu_ab_len = eintraege.len().max(grundlagen_eintragungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    eintraege.get(i).map(|bvz| format!("{}", bvz.get_lfd_nr())).unwrap_or_default(),
                    eintraege.get(i).map(|bvz| bvz.get_eigentuemer().text()).unwrap_or_default(),
                    grundlagen_eintragungen.get(i).map(|bvz| bvz.bv_nr.text()).unwrap_or_default(),
                    grundlagen_eintragungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::Abteilung1,
                geroetet: Geroetet::HalbHalb(
                    eintraege.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    grundlagen_eintragungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
                hvm_exception: None,
            });
        }
        
        
        let veraenderungen = if !mit_geroeteten_eintraegen {
            grundbuch.abt1.veraenderungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.abt1.veraenderungen.clone()
        };
        
        let loeschungen = if !mit_geroeteten_eintraegen {
            grundbuch.abt1.loeschungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.abt1.loeschungen.clone()
        };
        
        let zu_ab_len = veraenderungen.len().max(loeschungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    veraenderungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    veraenderungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::Abteilung1ZuAb,
                geroetet: Geroetet::HalbHalb(
                    veraenderungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
                hvm_exception: None,
            });
        }
    }
    
    if options.exportiere_abt2 {
    
        for abt2 in grundbuch.abt2.eintraege.iter() {
            
            if !mit_geroeteten_eintraegen && abt2.ist_geroetet() { continue; }
            
            rows.push(PdfTextRow {
                texts: vec![
                    format!("{}", abt2.lfd_nr),
                    abt2.bv_nr.text(),
                    abt2.text.text(),
                ],
                header: PdfHeader::Abteilung2,
                geroetet: Geroetet::Ganz(abt2.ist_geroetet()),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
                hvm_exception: None,
            });
        }
        
        let veraenderungen = if !mit_geroeteten_eintraegen {
            grundbuch.abt2.veraenderungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.abt2.veraenderungen.clone()
        };
        
        let loeschungen = if !mit_geroeteten_eintraegen {
            grundbuch.abt2.loeschungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.abt2.loeschungen.clone()
        };
        
        let zu_ab_len = veraenderungen.len().max(loeschungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    veraenderungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    veraenderungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::Abteilung2ZuAb,
                geroetet: Geroetet::HalbHalb(
                    veraenderungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
                hvm_exception: None,
            });
        }
    }
    
    if options.exportiere_abt3 {
        for abt3 in grundbuch.abt3.eintraege.iter() {
            
            if !mit_geroeteten_eintraegen && abt3.ist_geroetet() { continue; }

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
                force_single_line: Vec::new(),
                hvm_exception: None,
            });
        }
        
        let veraenderungen = if !mit_geroeteten_eintraegen {
            grundbuch.abt3.veraenderungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.abt3.veraenderungen.clone()
        };
        
        let loeschungen = if !mit_geroeteten_eintraegen {
            grundbuch.abt3.loeschungen
            .iter()
            .filter(|bvz| !bvz.ist_geroetet())
            .cloned()
            .collect()
        } else {
            grundbuch.abt3.loeschungen.clone()
        };
        
        let zu_ab_len = veraenderungen.len().max(loeschungen.len());
        
        for i in 0..zu_ab_len {
            rows.push(PdfTextRow {
                texts: vec![
                    veraenderungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    veraenderungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.lfd_nr.text()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.text.text()).unwrap_or_default(),
                ],
                header: PdfHeader::Abteilung3ZuAb,
                geroetet: Geroetet::HalbHalb(
                    veraenderungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                    loeschungen.get(i).map(|bvz| bvz.ist_geroetet()).unwrap_or_default(),
                ),
                teil_geroetet: BTreeMap::new(),
                force_single_line: Vec::new(),
                hvm_exception: None,
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

// Format a string so that it fits into N characters per line
fn wordbreak_text(s: &str, max_cols: usize) -> String {
    hyphenate(&unhyphenate(s), max_cols).join("\r\n")
}

lazy_static::lazy_static! {
    static ref DICT_DE: hyphenation::Standard = Standard::from_embedded(Language::German1996).unwrap();
    static ref REGEX_UNHYPHENATE: Regex = {
        regex::RegexBuilder::new("(.*)-\\s([a-züäö])(.*)")
                .multi_line(true)
                .case_insensitive(false)
                .build().unwrap()
    };
}

pub fn unhyphenate(text: &str) -> String {

    let mut und_saetze = text.lines().map(|s| {
        s.split("- und ")
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
    }).collect::<Vec<_>>();
    
    let mut text_sauber = String::new();
    
    for l in und_saetze.into_iter() {
        let und_len = l.len();
        for (index, mut s) in l.into_iter().enumerate() {
            while REGEX_UNHYPHENATE.is_match(&s) {
                s = REGEX_UNHYPHENATE.replace_all(&s, "$1$2$3").to_string();
            }
            text_sauber.push_str(&s);
            if index +1 != und_len {
                text_sauber.push_str("- und ");
            }
        }
    }
    
    text_sauber
}

pub fn hyphenate(text: &str, wrap_at_chars: usize) -> Vec<String> {
    let options = Options::new(wrap_at_chars)
        .word_splitter(WordSplitter::Hyphenation(DICT_DE.clone()));
    
    textwrap::wrap(text, &options)
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}
