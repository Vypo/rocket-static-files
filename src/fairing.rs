/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::hyper::header::{CacheControl, CacheDirective};
use rocket::http::{ContentType, Status};
use rocket::request::{FromRequest, Outcome};
use rocket::response::{Redirect, Responder, Result as ResponseResult};
use rocket::{Request, Rocket, State};

use serde::{Deserialize, Serialize};

use snafu::{ensure, OptionExt, ResultExt, Snafu};

use std::fmt::Display;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Snafu)]
#[non_exhaustive]
enum Error {
    /// Requested path not under `serve_from` path.
    OutOfBounds,

    /// Requested path not valid UTF-8.
    Utf8,

    /// An IO error occurred.
    Io { source: std::io::Error },
}

impl<'r> Responder<'r> for Error {
    fn respond_to(self, _: &Request) -> ResponseResult<'r> {
        match self {
            Error::Io { source } if source.kind() != io::ErrorKind::NotFound => {
                Err(Status::InternalServerError)
            }
            _ => Err(Status::NotFound),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    serve_from: PathBuf,
    path_prefix: String,
}

#[derive(Debug)]
struct Inner {
    config: Config,
    hashes: &'static phf::Map<&'static str, &'static str>,
}

/// Entry point for all of the functionality for `rocket-static-files`.
///
/// Attach the result of [`StaticFiles::fairing`] to your rocket.
///
/// Use `StaticFiles` as a request guard!
#[derive(Debug, Clone)]
pub struct StaticFiles(Arc<Inner>);

impl StaticFiles {
    /// Create a fairing to attach to your rocket instance:
    ///
    /// ```nocompile
    /// use rocket_static_files::StaticFiles;
    ///
    /// include!(concat!(env!("OUT_DIR"), "/static_file_hashes.rs"));
    ///
    /// fn main() {
    ///     rocket::ignite()
    ///         .attach(StaticFiles::fairing(&STATIC_FILE_HASHES))
    ///         .launch();
    /// }
    ///
    /// ```
    pub fn fairing(hashes: &'static phf::Map<&'static str, &'static str>) -> impl Fairing {
        StaticFilesFairing { hashes }
    }

    /// Compute the full path, including version hash if one exists.
    pub fn to<D: Display>(&self, path: D) -> String {
        let path = path.to_string();

        let hash = self
            .0
            .hashes
            .get(&path[1..])
            .map(|x| format!("?v={}", x))
            .unwrap_or_default();

        format!("{}{}{}", self.0.config.path_prefix, path, hash)
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for StaticFiles {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        request
            .guard::<State<StaticFiles>>()
            .map(|x| x.inner().clone())
    }
}

struct StaticFilesFairing {
    hashes: &'static phf::Map<&'static str, &'static str>,
}

impl Fairing for StaticFilesFairing {
    fn info(&self) -> Info {
        Info {
            name: "Static Files",
            kind: Kind::Attach,
        }
    }

    fn on_attach(&self, mut rocket: Rocket) -> Result<Rocket, Rocket> {
        let orig_config = match rocket.config().get_extra("static_files") {
            Ok(c) => c.clone(),
            Err(_) => return Err(rocket),
        };

        let orig_config: Config = match orig_config.try_into() {
            Ok(c) => c,
            Err(_) => return Err(rocket),
        };

        let canon = rocket
            .config()
            .root_relative(orig_config.serve_from)
            .canonicalize();

        let serve_from = match canon {
            Ok(s) => s,
            Err(_) => return Err(rocket),
        };

        rocket = rocket.mount(&orig_config.path_prefix, routes![serve_static]);

        Ok(rocket.manage(StaticFiles(Arc::new(Inner {
            hashes: self.hashes,
            config: Config {
                path_prefix: orig_config.path_prefix,
                serve_from,
            },
        }))))
    }
}

#[derive(Debug, Responder)]
struct FileResponse {
    file: File,
    content_type: ContentType,
    cache_control: CacheControl,
}

impl FileResponse {
    pub fn new<P: AsRef<Path>>(path: P, cache: bool) -> Result<Self, Error> {
        Self::new_path(path.as_ref(), cache)
    }

    fn cache_control(cache: bool) -> CacheControl {
        if cache {
            CacheControl(vec![CacheDirective::MaxAge(31536000)])
        } else {
            CacheControl(vec![])
        }
    }

    fn new_path(path: &Path, cache: bool) -> Result<Self, Error> {
        let file = File::open(path).context(Io)?;
        let mime = mime_guess::from_path(path).first_or_octet_stream();

        // TODO: Probably a better way to do this conversion
        let content_type = ContentType::from_str(&mime.to_string()).unwrap();

        Ok(FileResponse {
            file,
            content_type,
            cache_control: Self::cache_control(cache),
        })
    }
}

#[derive(Debug, Responder)]
enum RedirectOrFile {
    Redirect(Redirect),
    File(FileResponse),
}

#[get("/<path..>?<v>")]
fn serve_static(
    path: PathBuf,
    v: Option<String>,
    static_files: StaticFiles,
) -> Result<RedirectOrFile, Error> {
    let expected_revision = v.as_deref();

    let text = path.to_str().context(Utf8)?;
    let target = static_files
        .0
        .config
        .serve_from
        .join(&path)
        .canonicalize()
        .context(Io)?;

    ensure!(
        target.starts_with(&static_files.0.config.serve_from),
        OutOfBounds,
    );

    let current_revision = static_files.0.hashes.get(text).copied();

    let resp = match (expected_revision, current_revision) {
        (Some(expected), Some(current)) if expected == current => {
            RedirectOrFile::File(FileResponse::new(target, true)?)
        }
        (_, Some(current)) => {
            let url = format!(
                "{}{}",
                static_files.0.config.path_prefix,
                uri!(serve_static: path, current)
            );
            let redir = Redirect::to(url);
            RedirectOrFile::Redirect(redir)
        }
        (_, None) => RedirectOrFile::File(FileResponse::new(target, false)?),
    };

    Ok(resp)
}
