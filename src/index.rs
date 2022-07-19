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

use crate::{
    models::{get_data_dir, get_index_dir, PdfFile},
    MountPoint,
};
use std::path::Path;
use tantivy::schema::Schema;
use tantivy::schema::*;
use tantivy::{Index, IndexWriter};

static CACHE_FILE: &str = "CACHE.txt";

// /index/grundbuch
pub fn schema_grundbuch() -> Schema {
    let mut schema_builder = Schema::builder();
    let _ = schema_builder.add_text_field("id", STRING | STORED);
    let _ = schema_builder.add_text_field("land", STRING | STORED);
    let _ = schema_builder.add_text_field("amtsgericht", STRING | STORED);
    let _ = schema_builder.add_text_field("grundbuch_von", STRING | STORED);
    let _ = schema_builder.add_text_field("blatt", STRING | STORED);
    let _ = schema_builder.add_text_field("abteilung", STRING | STORED);
    let _ = schema_builder.add_text_field("lfd_nr", STRING | STORED);
    let _ = schema_builder.add_text_field("text", TEXT | STORED);
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
    use git2::{ObjectType, Repository, TreeWalkMode, TreeWalkResult};
    use std::collections::BTreeSet;

    let data_path = get_data_dir(MountPoint::Local);

    if !Path::new(&data_path).exists() {
        return Ok(()); // nichts zu tun
    }

    let repo = Repository::open(&data_path)
        .map_err(|e| format!("Fehler bei Indexierung von {data_path}: git_repository_open: {e}"))?;

    let reflog = repo.reflog("HEAD").map_err(|e| {
        format!("Fehler bei Indexierung von {data_path}: git_reflog(\"HEAD\"): {e}")
    })?;

    let commits = reflog
        .iter()
        .map(|rl| (rl.id_new(), rl.id_old()))
        .collect::<Vec<_>>();

    let _ = std::fs::create_dir_all(get_index_dir());
    let mut commits =
        match std::fs::read_to_string(Path::new(&get_index_dir()).join(CACHE_FILE)).ok() {
            None => commits.clone(),
            Some(s) => commits
                .iter()
                .take_while(|(new, old)| format!("{}", new) != s)
                .cloned()
                .collect(),
        };

    commits.reverse();

    let data_dir = get_data_dir(MountPoint::Local);
    let data_dir = Path::new(&data_dir);

    let (grundbuch_schema, grundbuch_index) = get_grundbuch_index()
        .map_err(|e| format!("Fehler in Index / Schema \"grundbuch\": {e}"))?;

    let mut index_writer = grundbuch_index
        .writer(100_000_000)
        .map_err(|e| format!("Fehler bei Allokation von 100MB für Schema \"grundbuch\": {e}"))?;

    let len = commits.len();

    let gemarkungen = crate::db::get_gemarkungen().unwrap_or_default();
    let mut dateien_indexed = BTreeSet::new();

    for (i, (new, _)) in commits.into_iter().enumerate() {
        let prozent_erledigt = (i as f32 / len as f32) * 100.0;

        println!("[{:02.2}%]\tIndexiere Commit {new}...", prozent_erledigt);

        let object = repo
            .find_object(new, Some(ObjectType::Commit))
            .map_err(|e| format!("Ungültige Objekt-ID: {new}: {e}"))?;

        let commit = object
            .as_commit()
            .ok_or(format!("Ungültige Änderungs-ID: {new}"))?;

        let tree = commit
            .tree()
            .map_err(|e| format!("Ungültige Änderungs-ID: {new}: {e}"))?;

        let mut files = Vec::new();

        let _ = tree
            .walk(TreeWalkMode::PreOrder, |path, entry| {
                if let Some(s) = entry.name() {
                    if s.ends_with(".gbx") {
                        files.push(format!("{path}{s}"));
                    }
                }

                TreeWalkResult::Ok
            })
            .map_err(|e| format!("Fehler in tree.walk(commit_id = {new}): {e}"))?;

        for f in files {
            let file_path = data_dir.join(&f);
            if dateien_indexed.contains(&file_path) {
                continue;
            }

            let pdf = std::fs::read_to_string(&file_path)
                .ok()
                .and_then(|f| serde_json::from_str::<PdfFile>(&f).ok());

            if let Some(s) = pdf.as_ref() {
                let land = gemarkungen.iter().find_map(|(land, ag, bezirk)| {
                    if *ag == s.analysiert.titelblatt.amtsgericht
                        && *bezirk == s.analysiert.titelblatt.grundbuch_von
                    {
                        Some(land.clone())
                    } else {
                        None
                    }
                });

                let land = land.ok_or(format!(
                    "Kein Land für Grundbuch {}_{}.gbx gefunden",
                    s.analysiert.titelblatt.grundbuch_von, s.analysiert.titelblatt.blatt
                ))?;

                add_grundbuchblatt_zu_index(&land, s, &index_writer, &grundbuch_schema)?;
                dateien_indexed.insert(file_path);
            }
        }

        let _ = index_writer
            .commit()
            .map_err(|e| format!("Fehler bei index.commit({new}): {e}"))?;

        let _ = std::fs::write(
            Path::new(&get_index_dir()).join(CACHE_FILE),
            &format!("{new}").as_bytes(),
        );
    }

    println!("OK: Indexierung abgeschlossen.");

    Ok(())
}

pub fn get_grundbuch_index() -> Result<(Schema, Index), String> {
    use tantivy::directory::MmapDirectory;

    let index_dir = get_index_dir();
    let schema = schema_grundbuch();

    let grundbuch_index_dir = Path::new(&index_dir).join("grundbuch");
    let _ = std::fs::create_dir_all(&grundbuch_index_dir);
    let dir = MmapDirectory::open(&grundbuch_index_dir)
        .map_err(|e| format!("Fehler beim Erstellen des Suchindex \"grundbuch\" (1): {e}"))?;
    let index = Index::open_or_create(dir, schema.clone())
        .map_err(|e| format!("Fehler beim Erstellen des Suchindex \"grundbuch\" (2): {e}"))?;

    Ok((schema, index))
}

pub fn get_commit_index() -> Result<Index, String> {
    use tantivy::directory::MmapDirectory;

    let index_dir = get_index_dir();
    let schema = schema_commits();

    let commits_index_dir = Path::new(&index_dir).join("commits");
    let _ = std::fs::create_dir_all(&commits_index_dir);
    let dir = MmapDirectory::open(&commits_index_dir)
        .map_err(|e| format!("Fehler beim Erstellen des Suchindex \"commits\" (1): {e}"))?;
    let index = Index::open_or_create(dir, schema.clone())
        .map_err(|e| format!("Fehler beim Erstellen des Suchindex \"commits\" (2): {e}"))?;

    Ok(index)
}

// Grundbuchblatt zu Suchindex hinzufügen
pub fn add_grundbuchblatt_zu_index(
    land_str: &str,
    pdf: &PdfFile,
    index_writer: &IndexWriter,
    schema: &Schema,
) -> Result<(), String> {
    use crate::models::BvEintrag;

    let file_name = format!(
        "{}/{}_{}.gbx",
        pdf.analysiert.titelblatt.amtsgericht,
        pdf.analysiert.titelblatt.grundbuch_von,
        pdf.analysiert.titelblatt.blatt
    );

    let blatt = format!("{}", pdf.analysiert.titelblatt.blatt);
    let blatt_id = format!(
        "Land {} AG {} GB von {} Blatt {}",
        land_str,
        pdf.analysiert.titelblatt.amtsgericht,
        pdf.analysiert.titelblatt.grundbuch_von,
        pdf.analysiert.titelblatt.blatt
    );

    let id = schema
        .get_field("id")
        .ok_or(format!("Kein Feld \"id\" in Schema \"grundbuch\""))?;
    let land = schema
        .get_field("land")
        .ok_or(format!("Kein Feld \"land\" in Schema \"grundbuch\""))?;
    let amtsgericht = schema
        .get_field("amtsgericht")
        .ok_or(format!("Kein Feld \"amtsgericht\" in Schema \"grundbuch\""))?;
    let grundbuch_von = schema.get_field("grundbuch_von").ok_or(format!(
        "Kein Feld \"grundbuch_von\" in Schema \"grundbuch\""
    ))?;
    let blatt = schema
        .get_field("blatt")
        .ok_or(format!("Kein Feld \"blatt\" in Schema \"grundbuch\""))?;
    let abteilung = schema
        .get_field("abteilung")
        .ok_or(format!("Kein Feld \"abteilung\" in Schema \"grundbuch\""))?;
    let lfd_nr = schema
        .get_field("lfd_nr")
        .ok_or(format!("Kein Feld \"lfd_nr\" in Schema \"grundbuch\""))?;
    let text = schema
        .get_field("text")
        .ok_or(format!("Kein Feld \"text\" in Schema \"grundbuch\""))?;

    // ... and add it to the `IndexWriter`.

    let _ = index_writer.delete_term(Term::from_field_text(id, &blatt_id));

    for bv in pdf.analysiert.bestandsverzeichnis.eintraege.iter() {
        match bv {
            BvEintrag::Flurstueck(bvf) => {
                let mut doc = Document::default();

                doc.add_text(id, &blatt_id);
                doc.add_text(land, land_str);
                doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
                doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
                doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
                doc.add_text(abteilung, "bv");
                doc.add_text(lfd_nr, format!("{}", bvf.lfd_nr));
                doc.add_text(
                    text,
                    format!(
                        "BV lfd. Nr. {}, Gemarkung {} Flur {} Flurstück {}: {}Größe: {} m²",
                        bvf.lfd_nr,
                        bvf.gemarkung
                            .clone()
                            .unwrap_or(pdf.analysiert.titelblatt.grundbuch_von.clone()),
                        bvf.flur,
                        bvf.flurstueck,
                        bvf.bezeichnung
                            .as_ref()
                            .map(|s| s.text_clean() + ", ")
                            .unwrap_or_default(),
                        bvf.groesse.get_m2(),
                    ),
                );

                index_writer
                    .add_document(doc)
                    .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
            }
            BvEintrag::Recht(bvr) => {
                let mut doc = Document::default();

                doc.add_text(id, &blatt_id);
                doc.add_text(land, land_str);
                doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
                doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
                doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
                doc.add_text(abteilung, "bv-herrschvermerke");
                doc.add_text(lfd_nr, format!("{}", bvr.lfd_nr));
                doc.add_text(
                    text,
                    format!(
                        "BV lfd. Nr. {} (zu lfd. Nr. {}): {}",
                        bvr.lfd_nr,
                        bvr.zu_nr.text_clean(),
                        bvr.text.text_clean(),
                    ),
                );

                index_writer
                    .add_document(doc)
                    .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
            }
        }
    }

    for bvz in pdf.analysiert.bestandsverzeichnis.zuschreibungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "bv-zuschreibungen");
        doc.add_text(lfd_nr, bvz.bv_nr.lines().join(" "));
        doc.add_text(
            text,
            format!(
                "BV-Zuschreibung zu lfd. Nr. {}: {}",
                bvz.bv_nr.lines().join(" "),
                bvz.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for bva in pdf.analysiert.bestandsverzeichnis.abschreibungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "bv-abschreibungen");
        doc.add_text(lfd_nr, bva.bv_nr.lines().join(" "));
        doc.add_text(
            text,
            format!(
                "BV-Abschreibung zu lfd. Nr. {}: {}",
                bva.bv_nr.lines().join(" "),
                bva.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for abt1 in pdf.analysiert.abt1.eintraege.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt1");
        doc.add_text(lfd_nr, format!("{}", abt1.get_lfd_nr()));
        doc.add_text(
            text,
            format!(
                "Abteilung 1, lfd. Nr. {}: {}",
                abt1.get_lfd_nr(),
                abt1.get_eigentuemer().text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for abt1 in pdf.analysiert.abt1.grundlagen_eintragungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt1-grundlagen-eintragungen");
        doc.add_text(lfd_nr, format!("{}", abt1.bv_nr.lines().join(" ")));
        doc.add_text(
            text,
            format!(
                "Abteilung 1, lfd. Nr. {}: {}",
                abt1.bv_nr.lines().join(" "),
                abt1.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for bvz in pdf.analysiert.abt1.veraenderungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt1-veraenderungen");
        doc.add_text(lfd_nr, bvz.lfd_nr.lines().join(" "));
        doc.add_text(
            text,
            format!(
                "Abteilung 1 Veränderung von lfd. Nr. {}: {}",
                bvz.lfd_nr.lines().join(" "),
                bvz.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for bva in pdf.analysiert.abt1.loeschungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt1-loeschungen");
        doc.add_text(lfd_nr, bva.lfd_nr.lines().join(" "));
        doc.add_text(
            text,
            format!(
                "Abteilung 1 Löschung von lfd. Nr. {}: {}",
                bva.lfd_nr.lines().join(" "),
                bva.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for abt2 in pdf.analysiert.abt2.eintraege.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt2");
        doc.add_text(lfd_nr, format!("{}", abt2.lfd_nr));
        doc.add_text(
            text,
            format!(
                "Abteilung 2, lfd. Nr. {}: {}",
                abt2.lfd_nr,
                abt2.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for bvz in pdf.analysiert.abt2.veraenderungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt2-veraenderungen");
        doc.add_text(lfd_nr, bvz.lfd_nr.lines().join(" "));
        doc.add_text(
            text,
            format!(
                "Abteilung 2 Veränderung von lfd. Nr. {}: {}",
                bvz.lfd_nr.lines().join(" "),
                bvz.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for bva in pdf.analysiert.abt2.loeschungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt2-loeschungen");
        doc.add_text(lfd_nr, bva.lfd_nr.lines().join(" "));
        doc.add_text(
            text,
            format!(
                "Abteilung 2 Löschung von lfd. Nr. {}: {}",
                bva.lfd_nr.lines().join(" "),
                bva.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for abt3 in pdf.analysiert.abt3.eintraege.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt3");
        doc.add_text(lfd_nr, format!("{}", abt3.lfd_nr));
        doc.add_text(
            text,
            format!(
                "Abteilung 3, lfd. Nr. {}: {}",
                abt3.lfd_nr,
                abt3.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for bvz in pdf.analysiert.abt3.veraenderungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt3-veraenderungen");
        doc.add_text(lfd_nr, bvz.lfd_nr.lines().join(" "));
        doc.add_text(
            text,
            format!(
                "Abteilung 3 Veränderung von lfd. Nr. {}: {}",
                bvz.lfd_nr.lines().join(" "),
                bvz.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    for bva in pdf.analysiert.abt3.loeschungen.iter() {
        let mut doc = Document::default();

        doc.add_text(id, &blatt_id);
        doc.add_text(land, land_str);
        doc.add_text(amtsgericht, &pdf.analysiert.titelblatt.amtsgericht);
        doc.add_text(grundbuch_von, &pdf.analysiert.titelblatt.grundbuch_von);
        doc.add_text(blatt, format!("{}", pdf.analysiert.titelblatt.blatt));
        doc.add_text(abteilung, "abt3-loeschungen");
        doc.add_text(lfd_nr, bva.lfd_nr.lines().join(" "));
        doc.add_text(
            text,
            format!(
                "Abteilung 3 Löschung von lfd. Nr. {}: {}",
                bva.lfd_nr.lines().join(" "),
                bva.text.text_clean()
            ),
        );

        index_writer
            .add_document(doc)
            .map_err(|e| format!("Konnte Dokument nicht indexieren: {file_name}: {e}"))?;
    }

    Ok(())
}
