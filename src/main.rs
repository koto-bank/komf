#![feature(plugin)]
#![plugin(maud_macros)]

extern crate iron;
extern crate staticfile;
extern crate mount;

extern crate rustbreak;
extern crate maud;
extern crate multipart;
extern crate sha1;
extern crate chrono;
#[macro_use] extern crate lazy_static;

use std::path::Path;
use std::fs::{File,copy,metadata,read_dir,remove_file};
use std::io::Read;
use std::str;

use iron::prelude::*;
use iron::status;
use staticfile::Static;
use mount::Mount;
use iron::headers::ContentLength;

use multipart::server::{Multipart,SaveResult};
use sha1::Sha1;
use rustbreak::Database;

use chrono::{Local,Duration,DateTime};


static FILE_DIR : &'static str = "files";
const MAX_MB : u64 = 512;
static MAX_SIZE : u64 = MAX_MB * 1024 * 1024; //512 MB

lazy_static!{
    static ref DB : Database<String> = Database::open("db.yaml").unwrap();
}

fn index(_: &mut Request) -> IronResult<Response> {
    let page = html!{
        link rel="stylesheet" href="/assets/main.css" /

        div.container {
            div.maincenter {
                h1#logo "Komf"
                form#upload action="/upload" method="POST" enctype="multipart/form-data" {
                    div.button {
                        span b "Select/Drop file here~"
                        input#file-input onchange="this.form.submit()" type="file" name="file" /
                    }
                    br
                    select name="date" {
                        option selected="selected" value="day" "Day"
                        option value="week" "Week"
                        option value="month" "Month"
                    }
                }
                {"File size limit is " (MAX_MB) "MB"}
            }
        }

    };
    Ok(Response::with((status::Ok, page)))
}

fn upload(req: &mut Request) -> IronResult<Response> {
    let headers = req.headers.clone();

    let size = match headers.get::<ContentLength>() { // In case somebody tries to send request w/o Content-Length
        Some(s) => s.0,
        None    => return Ok(Response::with((status::LengthRequired, "Length required!!1")))
    };

    if size > MAX_SIZE {
        return Ok(Response::with((status::Ok,"File is too large"))) // This actually just drops the connection
    }

    if let Ok(mut multipart) = Multipart::from_request(req) {
        match multipart.save_all() {
            SaveResult::Full(entries) | SaveResult::Partial(entries, _)  => {
                if let Some(savedfile) = entries.files.get("file") {
                    let ext = savedfile.filename.clone().unwrap();
                    let ext = ext.split('.').last().unwrap();

                    let mut body = Vec::new();
                    let _ = File::open(&savedfile.path).unwrap().read_to_end(&mut body);
                    let mut sha = Sha1::new();
                    sha.update(&body);
                    let name = sha.digest().to_string();

                    if let Err(_) = metadata(format!("{}/{}.{}", FILE_DIR, name, ext)) {
                        copy(&savedfile.path, format!("{}/{}.{}", FILE_DIR, name, ext)).unwrap();
                    }

                    let d = "day".to_owned();
                    let date = entries.fields.get("date").unwrap_or(&d);
                    let date = match date.as_str() {
                        "week" => Local::now() + Duration::weeks(1),
                        "month" => Local::now() + Duration::weeks(4),
                        "day" | _ => Local::now() + Duration::days(1)
                    };

                    DB.insert(&format!("{}.{}",name, ext),date).unwrap();
                    DB.flush().unwrap();

                    Ok(Response::with((status::Ok, format!("/file/{}.{}", name, ext))))
                } else { Ok(Response::with((status::BadRequest,"Can't load file/time"))) }
            },

            SaveResult::Error(e) =>  Ok(Response::with((status::BadRequest,format!("Couldn't handle POST! {:?}", e))))
        }
    } else {
        Ok(Response::with((status::BadRequest,"Not a multipart request?")))
    }
}

fn clean() {
    let list = read_dir(FILE_DIR).unwrap();
    for f in list {
        let file_path = f.unwrap();
        let file = file_path.file_name();
        let file = file.to_str().unwrap();
        if let Ok(date) = DB.retrieve::<DateTime<Local>,_>(file) {
            if Local::now() >= date {
                remove_file(file_path.path()).unwrap();
                DB.delete(file).unwrap();
                DB.flush().unwrap();
            }
        } else {
            remove_file(file_path.path()).unwrap();
        }
    }
}

fn main() {
    if std::env::args().any(|x| x == "clean") {
        clean()
    } else {
        let mut mount = Mount::new();
        mount.mount("/", index)
            .mount("/upload", upload)
            .mount("/file", Static::new(Path::new(FILE_DIR)))
            .mount("/assets", Static::new(Path::new("assets")));

        Iron::new(mount).http("127.0.0.1:3001").unwrap();
    }
}
