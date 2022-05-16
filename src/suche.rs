//! Suchfunktion:
//!
//! Die Index-Funktion durchsucht den jeweils letzten Stand des Suchindexes
//! bis zum jetzigen Stand des Repositories

const LOCK_FILE: &'static str = "AENDERUNG_STAND_IN_SUCHINDEX.txt";


pub struct SuchSchema {
    pub land: String,
    pub amtsgericht: String,
    pub grundbuch_von: String,
    pub blatt: usize,
    pub abteilung: String,
    pub spalte: String,
    pub text: String,
}

impl SuchSchema {

    pub fn get_tantivy_schema() -> tantivy::Schema {
        let mut schema_builder = Schema::builder();
        
        let _ = schema_builder.add_text_field("land", FAST | STORED | INDEXED);
        let _ = schema_builder.add_text_field("amtsgericht", FAST | STORED | INDEXED);
        let _ = schema_builder.add_text_field("grundbuch_von", FAST | STORED | INDEXED);
        let blatt = NumericOptions::default().set_stored().set_indexed();
        let _ = schema_builder.add_u64_field("blatt", blatt | INDEXED);
        let _ = schema_builder.add_text_field("abteilung", TEXT | STORED | INDEXED);
        let _ = schema_builder.add_text_field("spalte", TEXT | STORED | INDEXED);
        let _ = schema_builder.add_text_field("text", TEXT);
        
        schema_builder.build()
    }
    
    pub fn is_finished_building(&self) -> bool { 
        false 
    }
    
    pub fn is_finished_building_percent(&self) -> f32 {
        
    }
    
    pub fn get_modified_files(&self) -> Vec<String> {
        
    }
    
    pub fn start_indexing() -> JoinHandle<Result<(), String>> {
        std::thread::spawn(move || {
            let files_changed = 
        })
    }
    
    pub fn from_grundbuch(grundbuch: &PdfFile) -> Vec<SuchSchema> {
        
    }
}

pub fn add_document_to_index(last_revision: ) -> {
    
}

pub fn durchsuche_index() -> Vec<Suchergebnis> {

}
