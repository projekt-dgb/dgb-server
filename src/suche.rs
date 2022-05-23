//! Suchfunktion: durchsucht den momentanen Index

use serde_derive::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuchErgebnisse {
    pub grundbuecher: Vec<SuchErgebnisGrundbuch>,
    pub aenderungen: Vec<SuchErgebnisAenderung>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuchErgebnisAenderung {
    pub aenderungs_id: String,
    pub bearbeiter: String,
    pub datum: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuchErgebnisGrundbuch {
    pub land: String,
    pub amtsgericht: String,
    pub grundbuch_von: String,
    pub blatt: String,
    pub abteilung: String,
    pub lfd_nr: String,
    pub text: String,
}

pub fn suche_in_index(s: &str) -> Result<SuchErgebnisse, String> {

    use tantivy::query::QueryParser;
    use tantivy::{Score, DocAddress};
    use tantivy::collector::TopDocs;
 
    let (schema, index) = crate::index::get_grundbuch_index()
        .map_err(|e| format!("Suche: Konnte Index nicht erzeugen: {e}"))?;
    
    let land = schema.get_field("land").ok_or(format!("Kein Feld \"land\" in Schema \"grundbuch\""))?;
    let amtsgericht = schema.get_field("amtsgericht").ok_or(format!("Kein Feld \"amtsgericht\" in Schema \"grundbuch\""))?;
    let grundbuch_von = schema.get_field("grundbuch_von").ok_or(format!("Kein Feld \"grundbuch_von\" in Schema \"grundbuch\""))?;
    let blatt = schema.get_field("blatt").ok_or(format!("Kein Feld \"blatt\" in Schema \"grundbuch\""))?;
    let abteilung = schema.get_field("abteilung").ok_or(format!("Kein Feld \"abteilung\" in Schema \"grundbuch\""))?;
    let lfd_nr = schema.get_field("lfd_nr").ok_or(format!("Kein Feld \"lfd_nr\" in Schema \"grundbuch\""))?;
    let text = schema.get_field("text").ok_or(format!("Kein Feld \"text\" in Schema \"grundbuch\""))?;
    
    let reader = index.reader()
        .map_err(|e| format!("Suche: Konnte Reader nicht erzeugen: {e}"))?;

    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![text]);

    // QueryParser may fail if the query is not in the right
    // format. For user facing applications, this can be a problem.
    // A ticket has been opened regarding this problem.
    let query = query_parser.parse_query(s)
        .map_err(|e| format!("Fehler in Suchbegriff: {e}"))?;

    let top_docs: Vec<(Score, DocAddress)> = 
        searcher.search(&query, &TopDocs::with_limit(10))
        .map_err(|e| format!("Suche fehlgeschlagen: {e}"))?;
    
    let mut grundbuecher = Vec::new();
    
    for (_score, doc_address) in top_docs {
        
        let retrieved_doc = searcher.doc(doc_address)
            .map_err(|e| format!("Adresse {doc_address:?} nicht gefunden: {e}"))?;

        let land = retrieved_doc.get_first(land).and_then(|s| s.as_text()).map(|s| s.to_string())
            .ok_or(format!("Dokument {doc_address:?}: Fehlendes Feld \"land\""))?;
        let amtsgericht = retrieved_doc.get_first(amtsgericht).and_then(|s| s.as_text()).map(|s| s.to_string())
            .ok_or(format!("Dokument {doc_address:?}: Fehlendes Feld \"amtsgericht\""))?;
        let grundbuch_von = retrieved_doc.get_first(grundbuch_von).and_then(|s| s.as_text()).map(|s| s.to_string())
            .ok_or(format!("Dokument {doc_address:?}: Fehlendes Feld \"grundbuch_von\""))?;
        let blatt = retrieved_doc.get_first(blatt).and_then(|s| s.as_text()).map(|s| s.to_string())
            .ok_or(format!("Dokument {doc_address:?}: Fehlendes Feld \"blatt\""))?;
        let abteilung = retrieved_doc.get_first(abteilung).and_then(|s| s.as_text()).map(|s| s.to_string())
            .ok_or(format!("Dokument {doc_address:?}: Fehlendes Feld \"abteilung\""))?;
        let lfd_nr = retrieved_doc.get_first(lfd_nr).and_then(|s| s.as_text()).map(|s| s.to_string())
            .ok_or(format!("Dokument {doc_address:?}: Fehlendes Feld \"lfd_nr\""))?;
                
        let text = retrieved_doc.get_first(text).and_then(|s| s.as_text()).map(|s| s.to_string())
            .ok_or(format!("Dokument {doc_address:?}: Fehlendes Feld \"text\""))?;
        
        grundbuecher.push(SuchErgebnisGrundbuch {
            land: land,
            amtsgericht: amtsgericht,
            grundbuch_von: grundbuch_von,
            blatt: blatt,
            abteilung: abteilung,
            lfd_nr: lfd_nr,
            text: text,
        });
    }

    let aenderungen = Vec::new();
    
    Ok(SuchErgebnisse {
        grundbuecher,
        aenderungen,
    })
}
