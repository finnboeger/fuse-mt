use anyhow::{Context, Result};
use diesel::prelude::*;
use diesel::connection::SimpleConnection;
use image::GenericImageView;
#[cfg(feature = "mount")]
use indicatif::{ProgressBar, ProgressIterator};

use std::{
    io::{Seek, Write},
    time::SystemTime,
    path::{Path, PathBuf}
};

table! {
    #[allow(non_snake_case)]
    Cover (ID) {
        ID -> Integer,
        Filename -> Text,
        #[sql_name = "Date"]
        CreationDate -> Integer,
        Width -> Integer,
        Height -> Integer,
    }
}

table! {
    #[allow(non_snake_case)]
    CoverThumbnail (ID) {
        ID -> Integer,
        Format -> Integer,
        Width -> Integer,
        Height -> Integer,
        Data -> Integer,
    }
}

allow_tables_to_appear_in_same_query!(
    Cover,
    CoverThumbnail,
);

// Default Thumbnail format in USDX
const TEXTURE_FORMAT: i32 = 1; //`ipfRGB` in USDX
// https://github.com/UltraStar-Deluxe/USDX/blob/master/src/base/UCovers.pas#L456
// https://github.com/UltraStar-Deluxe/USDX/blob/4849669cae06421369430c56c7e302f43fc47713/src/base/UImage.pas#L50

pub struct CoverDB {
    dbfile: tempfile::NamedTempFile,
    conn: diesel::sqlite::SqliteConnection,
    relative_to: PathBuf,
}

impl CoverDB {
    pub fn new<P: AsRef<Path>>(relative: P) -> Result<CoverDB> {
        let temp = tempfile::NamedTempFile::new().context("Unable to open temporary cover.db file")?;
        let conn = diesel::sqlite::SqliteConnection::establish(temp.path().to_str().expect("NamedFile path is no valid UTF-8"))?;
        conn.batch_execute(include_str!("init.sql")).context("Failed to initialize database")?;
        Ok(CoverDB {
            dbfile: temp,
            conn,
            relative_to: PathBuf::from(relative.as_ref()),
        })
    }

    pub fn add<P: AsRef<Path>>(&mut self, cover: P) -> Result<()> {
        let cover = cover.as_ref();

        self.conn.transaction(|| {
            let image = image::open(cover).with_context(|| format!("Unable to load image file '{}'", cover.display()))?;
            let mut file_name = cover.strip_prefix(&self.relative_to).with_context(|| format!("Cover '{}' is not relative to src_dir", cover.display()))?
                .to_str().with_context(|| format!("Unable to store filename '{}' in database", cover.display()))?.to_string();
            // Add null byte at the end since usdx is weird.
            file_name.push(char::from(0));
            diesel::insert_into(Cover::table)
                .values((
                    Cover::Filename.eq(&file_name),
                    Cover::CreationDate.eq(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("SystemTime before unix epoch").as_secs() as i32),
                    Cover::Width.eq(image.width() as i32),
                    Cover::Height.eq(image.height() as i32),
                ))
                .execute(&self.conn).with_context(|| format!("Unable to add cover to database '{}'", cover.display()))?;
            
            let id: i32 = Cover::table.select(Cover::ID).order(Cover::ID.desc()).first(&self.conn).context("Unable to get ID of cover")?;
            // the database fields needs to be uncompressed/non-overlapping
            diesel::insert_into(CoverThumbnail::table)
                .values((
                    CoverThumbnail::ID.eq(id),
                    CoverThumbnail::Format.eq(TEXTURE_FORMAT),
                    CoverThumbnail::Width.eq(image.width() as i32),
                    CoverThumbnail::Height.eq(image.height() as i32),
                    CoverThumbnail::Data.eq(0),
                ))
                .execute(&self.conn).with_context(|| format!("Unable to add cover to database '{}'", cover.display()))?;
            
            Ok(())
        })
    }

    pub fn write<W: Write>(mut self, mut target: W) -> Result<()> {
        std::mem::drop(self.conn);
        self.dbfile.flush()?;
        self.dbfile.seek(std::io::SeekFrom::Start(0))?;
        std::io::copy(&mut self.dbfile, &mut target).context("Unable to write cover.db").map(|_| ())?;
        Ok(())
    }
}

#[cfg(feature = "mount")]
pub fn import<P1: AsRef<Path>, P2: AsRef<Path>, P3: AsRef<Path>>(cache: P1, dest: P2, base: P3) -> Result<()> {
    let src = diesel::sqlite::SqliteConnection::establish(cache.as_ref().to_str().expect("src database path is no valid UTF-8"))?;
    let db_exists = dest.as_ref().exists();
    let dest = diesel::sqlite::SqliteConnection::establish(dest.as_ref().to_str().expect("dest database path is no valid UTF-8"))?;
    if !db_exists {
        dest.batch_execute(include_str!("init.sql")).context("Failed to initialize database")?;
    }
    let base = base.as_ref();

    info!("Importing cover.db");
    let covers = Cover::table.load::<(i32, String, i32, i32, i32)>(&src).context("Failed to load table Cover from cache cover.db")?;
    let pb = ProgressBar::new(covers.len() as u64);
    let pb_err = pb.clone();

    for cover in covers.into_iter().progress_with(pb) {
        let old_id = cover.0;
        let file_path = base.join(&cover.1);
        let file = file_path.to_str().with_context(|| format!("Unable to represent new filename as UTF-8: {}", file_path.display()))?;

        if let Err(diesel::result::Error::NotFound) | Ok(0)  = Cover::table.filter(Cover::Filename.eq(&file)).count().get_result::<i64>(&dest) {
            if let Err(err) = dest.transaction(|| -> Result<()> {
                diesel::insert_into(Cover::table)
                .values((
                    Cover::Filename.eq(file),
                    Cover::CreationDate.eq(cover.2),
                    Cover::Width.eq(cover.3),
                    Cover::Height.eq(cover.4),
                ))
                .execute(&dest).with_context(|| format!("Unable to add cover to database '{}'", old_id))?;

                let new_id: i32 = Cover::table.select(Cover::ID).order(Cover::ID.desc()).first(&dest).with_context(|| format!("Unable to get new ID of cover {}", old_id))?;
                let cover_thumbnail = CoverThumbnail::table.find(old_id).first::<(i32, i32, i32, i32, i32)>(&src).with_context(|| format!("Unable to find CoverThumbnail for {}", old_id))?;
                
                diesel::insert_into(CoverThumbnail::table)
                .values((
                    CoverThumbnail::ID.eq(new_id),
                    CoverThumbnail::Format.eq(cover_thumbnail.1),
                    CoverThumbnail::Width.eq(cover_thumbnail.2),
                    CoverThumbnail::Height.eq(cover_thumbnail.3),
                    CoverThumbnail::Data.eq(cover_thumbnail.4),
                ))
                .execute(&dest).with_context(|| format!("Unable to add thumbnail to database '{}'", old_id))?;

                Ok(())
            }) {
                pb_err.println(format!("Error importing '{}'({}): {}", cover.0, &cover.1, err));
            }
        }
    }

    Ok(())
}