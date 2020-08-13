use anyhow::{Context, Result};
use diesel::prelude::*;
use image::{GenericImageView, Pixel};

use std::{
    io::{Seek, Write},
    time::SystemTime,
    path::{Path, PathBuf}
};
use image::imageops::FilterType;

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
        Data -> Nullable<Binary>,
    }
}

allow_tables_to_appear_in_same_query!(
    Cover,
    CoverThumbnail,
);

// Default Thumbnail size in USDX (TODO: make this configurable?)
const TEXTURE_WIDTH: i32 = 256;
const TEXTURE_HEIGHT: i32 = 256;
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
        diesel::sql_query(include_str!("init.sql")).execute(&conn).context("Failed to initialize database")?;
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
            diesel::insert_into(Cover::table)
                .values((
                    Cover::Filename.eq(
                        cover.strip_prefix(&self.relative_to).with_context(|| format!("Cover '{}' is not relative to src_dir", cover.display()))?
                        .to_str().with_context(|| format!("Unable to store filename '{}' in database", cover.display()))?),
                    Cover::CreationDate.eq(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("SystemTime before unix epoch").as_secs() as i32),
                    Cover::Width.eq(image.width() as i32),
                    Cover::Height.eq(image.height() as i32),
                ))
                .execute(&self.conn).with_context(|| format!("Unable to add cover to database '{}'", cover.display()))?;
            
            let id: i32 = Cover::table.select(Cover::ID).order(Cover::ID.desc()).first(&self.conn).context("Unable to get ID of cover")?;
            // the database fields needs to be uncompressed/non-overlapping
            let thumbnail: Vec<u8> = image.resize_to_fill(TEXTURE_WIDTH as u32, TEXTURE_HEIGHT as u32, FilterType::Triangle).into_rgb()
                .pixels().flat_map(|pixel| pixel.channels()).copied().collect();
            diesel::insert_into(CoverThumbnail::table)
                .values((
                    CoverThumbnail::ID.eq(id),
                    CoverThumbnail::Format.eq(TEXTURE_FORMAT),
                    CoverThumbnail::Width.eq(TEXTURE_WIDTH),
                    CoverThumbnail::Height.eq(TEXTURE_HEIGHT),
                    CoverThumbnail::Data.eq(Some(&thumbnail)),
                ))
                .execute(&self.conn).with_context(|| format!("Unable to add cover to database '{}'", cover.display()))?;
            
            Ok(())
        })
    }

    pub fn write<W: Write>(mut self, mut target: W) -> Result<()> {
        std::mem::drop(self.conn);
        self.dbfile.seek(std::io::SeekFrom::Start(0))?;
        std::io::copy(&mut self.dbfile, &mut target).context("Unable to write cover.db").map(|_| ())
    }
}