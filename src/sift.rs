//! Utilities to load the SIFT dataset.
//!
//! <http://corpus-texmex.irisa.fr>

use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read};
use std::path::Path;

use flechasdb::vector::BlockVectorSet;

use crate::error::Error;

/// Vector size.
pub const VECTOR_SIZE: usize = 128;

/// Reads `fvecs` data.
///
/// # `fvecs` file structure
///
/// 1. [`u32`]: vector size
/// 2. [`f32`]: vector elements. vector size * number of vectors.
///
/// The number of vectors is determined from the file size.
pub fn read_fvecs(mut read: impl Read) -> Result<BlockVectorSet<f32>, Error> {
    // reads the first vector to know the vector size
    let vector_size = read.read_u32::<LittleEndian>()? as usize;
    if vector_size != VECTOR_SIZE {
        return Err(Error::InvalidData(format!(
            "invalid vector size: expected {} but got {}",
            VECTOR_SIZE,
            vector_size,
        )));
    }
    let mut block: Vec<f32> = Vec::with_capacity(vector_size * 1_000_000);
    let mut vector_buf: Vec<f32> = Vec::with_capacity(vector_size);
    unsafe { vector_buf.set_len(vector_size); }
    read.read_f32_into::<LittleEndian>(&mut vector_buf)?;
    block.extend_from_slice(&vector_buf);
    // reads all the remaining vectors
    loop {
        let d = match read.read_u32::<LittleEndian>() {
            Ok(value) => value as usize,
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };
        if d != vector_size {
            return Err(Error::InvalidData(format!(
                "inconsistent vector size: expected {} but got {}",
                vector_size,
                d,
            )));
        }
        read.read_f32_into::<LittleEndian>(&mut vector_buf)?;
        block.extend_from_slice(&vector_buf);
    }
    Ok(BlockVectorSet::chunk(block, vector_size.try_into().unwrap())?)
}

/// Reads a given `fvecs` file.
pub fn read_fvecs_file(
    path: impl AsRef<Path>,
) -> Result<BlockVectorSet<f32>, Error> {
    let f = File::open(path)?;
    read_fvecs(BufReader::new(f))
}
