/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub use crate::gen::error::Error;

use phf_codegen::Map;

use siphasher::sip::SipHasher;

use snafu::{OptionExt, ResultExt, Snafu};

use std::collections::HashMap;
use std::fs::File;
use std::hash::Hasher;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

mod error {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    pub enum Error {
        WalkDir { source: walkdir::Error },
        Io { source: std::io::Error },
        Unprintable { path: PathBuf },
    }
}

fn hash(path: &Path) -> Result<u64, Error> {
    let mut file = File::open(path).context(error::Io)?;
    let mut hasher = SipHasher::new();

    let mut buffer = [0u8; 1024];

    while let Ok(read) = file.read(&mut buffer) {
        if read == 0 {
            break;
        }

        hasher.write(&buffer[0..read]);
    }

    Ok(hasher.finish())
}

fn rerun(path: &Path) -> Result<(), Error> {
    let txt = path.to_str().with_context(|| error::Unprintable {
        path: path.to_owned(),
    })?;

    println!("cargo:rerun-if-changed={}", txt);
    Ok(())
}

pub fn generate(out_path: &Path, static_root: &Path) -> Result<(), Error> {
    let mut files = HashMap::new();

    for entry_res in WalkDir::new(static_root).into_iter() {
        let entry = entry_res.context(error::WalkDir)?;
        rerun(entry.path())?;

        if !entry.file_type().is_file() {
            continue;
        }

        let file_hash = hash(entry.path())?;
        let rel_path = entry.path().strip_prefix(static_root).unwrap();
        let rel_str = rel_path.to_str().with_context(|| error::Unprintable {
            path: rel_path.to_owned(),
        })?;

        files.insert(rel_str.to_owned(), file_hash);
    }

    let refs: HashMap<_, _> = files.iter().map(|(k, v)| (k.as_str(), v)).collect();

    let mut map = Map::new();
    map.phf_path("::rocket_static_files::phf");
    for (key, value) in refs {
        let hashed = base64::encode_config(value.to_le_bytes(), base64::URL_SAFE_NO_PAD);
        map.entry(key, &format!("\"{}\"", hashed));
    }

    let output = map.build();

    let mut out_file = File::create(out_path).context(error::Io)?;
    write!(
        out_file,
        "static STATIC_FILE_HASHES: ::rocket_static_files::phf::Map<&'static str, &'static str> = {};",
        output,
    )
    .context(error::Io)?;

    Ok(())
}
