//! Indexierung des Repositorys mit tantivy und git
//!
//! Bei der Indexierung wird die gesamte History des
//! Repositorys durchsucht (git reflog) und alle Dateien
//! werden nach und nach dem Index hinzugefügt.
//! 
//! Der aktuelle Stand des Commit-Hashs wird in /index/STATUS.txt
//! abgelegt, damit bei einer Neu-Indexierung nicht noch einmal
//! alle Dateien durchsucht werden müssen.
//!
//! Die Indices sind für verschiedene Ausgaben optimiert, z.B.
//! kann man mit /index/grundbuch die Grundbuchblätter durchsuchen
//! während man mit /index/commits die Versionsgeschichte 
//! durchsuchen kann.

use crate::models::{PdfFile, get_index_dir, get_data_dir};
use tantivy::schema::{Schema, SchemaBuilder};
use tantivy::schema::*;
use tantivy::{Index, IndexWriter};
use std::path::Path;

static CACHE_FILE: &str = "CACHE.txt";

// /index/grundbuch
pub fn schema_grundbuch() -> Schema {
    let mut schema_builder = Schema::builder();
    let _ = schema_builder.add_text_field("land", STRING | STORED);
    let _ = schema_builder.add_text_field("amtsgericht", STRING | STORED);
    let _ = schema_builder.add_text_field("gemarkung", STRING | STORED);
    let _ = schema_builder.add_text_field("blatt", STRING | STORED);
    let _ = schema_builder.add_text_field("abteilung", STRING | STORED);
    let _ = schema_builder.add_text_field("spalte", STRING | STORED);
    let _ = schema_builder.add_text_field("lfd_nr", STRING | STORED);
    let _ = schema_builder.add_text_field("text", TEXT);
    schema_builder.build()
}

// /index/grundbuch
pub fn schema_commits() -> Schema {
    let mut schema_builder = Schema::builder();
    let _ = schema_builder.add_text_field("commit_id", STRING | STORED);
    let _ = schema_builder.add_text_field("dateipfade", STRING | STORED);
    let _ = schema_builder.add_text_field("titel", TEXT);
    let _ = schema_builder.add_text_field("beschreibung", TEXT);
    schema_builder.build()
}

pub fn index_all() -> Result<(), String> {
    
    use git2::{Repository, TreeWalkMode, TreeWalkResult, ObjectType};

    let data_path = get_data_dir();
    
    if !Path::new(&data_path).exists() {
        return Ok(()); // nichts zu tun
    }
    
    let repo = Repository::open(&data_path)
        .map_err(|e| format!("Fehler bei Indexierung von {data_path}: git_repository_open: {e}"))?;
    
    let reflog = repo.reflog("HEAD")
        .map_err(|e| format!("Fehler bei Indexierung von {data_path}: git_reflog(\"HEAD\"): {e}"))?;
    
    let commits = reflog
        .iter()
        .map(|rl| (rl.id_new(), rl.id_old()))
        .collect::<Vec<_>>();
    
    let _ = std::fs::create_dir_all(get_index_dir());
    let mut commits = match std::fs::read_to_string(Path::new(&get_index_dir()).join(CACHE_FILE)).ok() {
        None => commits.clone(),
        Some(s) => commits.iter().take_while(|(new, old)| format!("{}", new) != s).cloned().collect(), 
    };
    
    commits.reverse();
    
    let data_dir = get_data_dir();
    let data_dir = Path::new(&data_dir);
    
    let grundbuch_index = get_grundbuch_index()
        .map_err(|e| format!("Fehler in Index / Schema \"grundbuch\": {e}"))?;
        
    let mut index_writer = grundbuch_index.writer(100_000_000)
        .map_err(|e| format!("Fehler bei Allokation von 100MB für Schema \"grundbuch\": {e}"))?;

    let len = commits.len();
    
    for (i, (new, _)) in commits.into_iter().enumerate() {
        
        let prozent_erledigt = (i as f32 / len as f32) * 100.0;
        println!("[{prozent_erledigt:.2}%]\tIndexiere Commit {new}...");
        
        let object = repo.find_object(new, Some(ObjectType::Commit))
            .map_err(|e| format!("Ungültige Objekt-ID: {new}: {e}"))?;
        
        let commit = object.as_commit()
            .ok_or(format!("Ungültige Änderungs-ID: {new}"))?;
        
        let tree = commit.tree()
            .map_err(|e| format!("Ungültige Änderungs-ID: {new}: {e}"))?;
        
        let mut files = Vec::new();
        
        let _ = tree.walk(TreeWalkMode::PreOrder, |path, entry| {
        
            if let Some(s) = entry.name() {
                if s.ends_with(".gbx") {
                    files.push(format!("{path}{s}"));
                }
            }
            
            TreeWalkResult::Ok
        }).map_err(|e| format!("Fehler in tree.walk(commit_id = {new}): {e}"))?;
        
        for f in files {
            let file_path = data_dir.join(&f);
            let pdf = std::fs::read_to_string(&file_path).ok()
                .and_then(|f| serde_json::from_str::<PdfFile>(&f).ok());
                
            if let Some(s) = pdf.as_ref() {
                add_grundbuchblatt_zu_index(s, &index_writer)?;
            }
        }
        
        let _ = index_writer.commit()
            .map_err(|e| format!("Fehler bei index.commit({new}): {e}"))?;
    }
    
    println!("OK: Indexierung abgeschlossen.");
    Ok(())
}

pub fn get_grundbuch_index() -> Result<Index, String> {

    use tantivy::directory::MmapDirectory;
    
    let index_dir = get_index_dir();
    
    let grundbuch_index_dir = Path::new(&index_dir).join("grundbuch");
    let _ = std::fs::create_dir_all(&grundbuch_index_dir);
    let dir = MmapDirectory::open(&grundbuch_index_dir)
        .map_err(|e| format!("Fehler beim Erstellen des Suchindex \"grundbuch\""))?;
    let index = Index::open_or_create(dir, schema_grundbuch())
        .map_err(|e| format!("Fehler beim Erstellen des Suchindex \"grundbuch\""))?;
    
    Ok(index)
}

pub fn get_commit_index() -> Result<Index, String> {
    
    use tantivy::directory::MmapDirectory;

    let index_dir = get_index_dir();
    
    let commit_index_dir = Path::new(&index_dir).join("grundbuch");
    let _ = std::fs::create_dir_all(&commit_index_dir);
    let dir = MmapDirectory::open(&commit_index_dir)
        .map_err(|e| format!("Fehler beim Erstellen des Suchindex \"grundbuch\""))?;
    let index = Index::open_or_create(dir, schema_commits())
        .map_err(|e| format!("Fehler beim Erstellen des Suchindex \"grundbuch\""))?;
    
    Ok(index)
}

// Grundbuchblatt zu Suchindex hinzufügen
fn add_grundbuchblatt_zu_index(pdf: &PdfFile, index_writer: &IndexWriter) -> Result<(), String>  {
    let file_name = format!("{}/{}_{}.gbx", pdf.titelblatt.amtsgericht, pdf.titelblatt.grundbuch_von, pdf.titelblatt.blatt);
    
    /*
    index_writer.add_document(doc!(
        land => ,
        amtsgericht => ,
        gemarkung => ,
        blatt => ,
        abteilung => ,
        spalte => ,
        lfd_nr => ,
        text => ,
    )).map_err(|e| format!("Fehler bei Indexierung von {file_name}: {e}"));
    */
    Ok(())
}
