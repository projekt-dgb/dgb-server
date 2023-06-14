use std::path::PathBuf;

pub fn write_files_to_zip(files: &[(Option<String>, PathBuf, Vec<u8>)]) -> Vec<u8> {
    use std::io::Cursor;
    use std::io::Write;
    use zip::write::{FileOptions, ZipWriter};

    let mut cursor = Cursor::new(Vec::new());

    {
        let mut zip = ZipWriter::new(&mut cursor);

        for (option_dir, path_buf, file_contents) in files.iter() {
            let path = path_buf.as_path();
            let name = path;

            let path_buf = if let Some(dir) = option_dir {
                PathBuf::from(format!("{}/{}", dir, name.display()))
            } else {
                PathBuf::from(format!("/{}", name.display()))
            };

            let path = path_buf.as_path();
            let name = path;

            let options = FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o755);

            if let Some(dir) = option_dir {
                #[allow(deprecated)]
                zip.add_directory(dir, options).unwrap();
            }

            #[allow(deprecated)]
            zip.start_file_from_path(name, options).unwrap();
            zip.write_all(&file_contents).unwrap();
        }

        zip.finish().unwrap();
    }

    cursor.into_inner()
}