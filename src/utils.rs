#[cfg(feature = "novideo")]
use anyhow::{anyhow, Context, Result};
use std::path::Path;

pub fn path_to_rel(path: &Path) -> &Path {
    if path.starts_with("/") {
        path.strip_prefix("/").unwrap()
    } else if path.starts_with("./") {
        path.strip_prefix("./").unwrap()
    } else {
        path
    }
}

#[cfg(feature = "novideo")]
pub fn read_txt_from_buf<B: AsRef<[u8]>>(buf: B) -> Result<ultrastar_txt::TXTSong> {
    let reader = buf.as_ref();

    // detect encoding and decode to String
    let chardet_result = chardet::detect(&reader);
    let whtwg_label = chardet::charset2encoding(&chardet_result.0);
    let coder = encoding::label::encoding_from_whatwg_label(whtwg_label);
    let txt = match coder {
        Some(c) => match c.decode(&reader, encoding::DecoderTrap::Ignore) {
            Ok(x) => x,
            Err(e) => return Err(e).map_err(|x| anyhow!("{}", x)).context("Error decoding"),
        },
        None => return Err(anyhow!("Failed to detect encoding")),
    };

    Ok(ultrastar_txt::TXTSong {
        header: ultrastar_txt::parse_txt_header_str(txt.as_ref()).map_err(|x| anyhow!("{}", x)).context("Failed to parse header")?,
        lines: ultrastar_txt::parse_txt_lines_str(txt.as_ref()).map_err(|x| anyhow!("{}", x)).context("Failed to parse lines")?,
    })
}