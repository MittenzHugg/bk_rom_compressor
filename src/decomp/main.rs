use std::env;
use std::fs::{self};
use std::io::Write;
use md5;
use rarezip;




#[derive(Debug)]
pub enum GameVersion {
    USA,
    PAL,
    JP,
    USARevA,
}

#[derive(Debug)]
pub enum GameId {
    BanjoKazooie(GameVersion),
}

#[derive(Debug)]
pub enum ROMEndianessError {
    NonN64ROM,
}

fn get_hash(rom : &Vec<u8>) -> Result<GameId, md5::Digest> {
    let digest = md5::compute(rom);
    match format!("{:x}", digest).as_str() {
        "b29599651a13f681c9923d69354bf4a3" => Ok(GameId::BanjoKazooie(GameVersion::USA)),
        "06a43bacf5c0687f596df9b018ca6d7f" => Ok(GameId::BanjoKazooie(GameVersion::PAL)),
        "3d3855a86fd5a1b4d30beb0f5a4a85af" => Ok(GameId::BanjoKazooie(GameVersion::JP)),
        "b11f476d4bc8e039355241e871dc08cf" => Ok(GameId::BanjoKazooie(GameVersion::USARevA)),
        _ => Err(digest)
    }
}

fn le_to_me(le_buff : Vec<u8>) -> Vec<u8> {
    le_buff.chunks_exact(2)
    .map(|a|{[a[1], a[0]]})
    .flatten()
    .collect()
}

fn le_to_be(le_buff : Vec<u8>) -> Vec<u8> {
    le_buff.chunks_exact(4)
        .map(|a|{[a[3], a[2], a[1], a[0]]})
        .flatten()
        .collect()
}

fn rom_to_big_endian(rom_bin : Vec<u8>) -> Result<Vec<u8>, ROMEndianessError> {
    let signature = &rom_bin[0..4];
    match signature {
        [0x80, 0x37, 0x12, 0x40] => {Ok(rom_bin)},
        [0x40, 0x12, 0x37, 0x80] => {Ok(le_to_be(rom_bin))},
        [0x37, 0x80, 0x40, 0x12] => {Ok(le_to_me(rom_bin))},
        _ => Err(ROMEndianessError::NonN64ROM),
    }
}

fn main() {
    let help_text = include_str!("decomp_help.txt");

    //get rom in_path
    let mut shell_arg = env::args();
    let _cmd = shell_arg.next(); //consume function call
    let source_path = shell_arg.next().expect(format!("No compressed ROM path specified\n\n{}", help_text).as_str());
    let target_path = shell_arg.next().expect(format!("No destination ROM path specified\n\n{}", help_text).as_str());

    //check input file exists
    assert!(fs::metadata(&source_path).unwrap().is_file(), "Input \"{}\" not found", source_path);

    //create output
    //println!("Decompressing ROM {} => {}", source_path, target_path);
    
    //read in binary and convert to big endian
    let compressed_rom : Vec<u8> = fs::read(source_path).expect("Could not read file \"{}\"");
    let compressed_rom = rom_to_big_endian(compressed_rom).expect("Error converting rom to big endian");

    //check game version ?
    let game_id = get_hash(&compressed_rom).expect("Unsupported game hash");
    //println!("Game Identified as {:?}", game_id);

    //get all file offsets
    let file_offsets : Vec<usize> = match game_id {
        /* ToDo include all 4 versions*/
        GameId::BanjoKazooie(GameVersion::USA) => vec!(
            /*core1*/   0xF19250, 0xF19250 + 0x1D09B, 
            /*core2*/   0xF37F90, 0xF9CAE0 + 0x64B50, 
            /*whale*/   0xFA3FD0, 0xFA3FD0 + 0x1DC6,
            /*haunted*/ 0xFA5F50, 0xFA5F50 + 0x2D96,
            /*desert*/  0xFA9150, 0xFA9150 + 0x512E,
            /*beach*/   0xFAE860, 0xFAE860 + 0x328B,
            /*jungle*/  0xFB24A0, 0xFB24A0 + 0x1E39,
            /*swamp*/   0xFB44E0, 0xFB44E0 + 0x5130,
            /*ship*/    0xFB9A30, 0xFB9A30 + 0x4BB2,
            /*snow*/    0xFBEBE0, 0xFBEBE0 + 0x540F,
            /*training*/ 0xFC4810, 0xFC4810 + 0x23FF,
            /*intro*/   0xFC6F20, 0xFC6F20 + 0x1BDC,
            /*witch*/   0xFC9150, 0xFC9150 + 0x6548,
            /*battle*/  0xFD0420, 0xFD0420 + 0x5640,
            /*tree*/    0xFD6190, 0xFD6190 + 0x416F,
            /*coshow*/  0xFDAA10, 0xFDAA10 + 0xE,
            0xFDAA30
        ),
        GameId::BanjoKazooie(GameVersion::PAL) => vec!(
            /*core1*/    0xF3D980, 0xF3D980 + 0x1C95C, 
            /*core2*/    0xF5BEC0, 0xF5BEC0 + 0x64E3D,
            /*whale*/    0xFC8460, 0xFC8460 + 0x1DB1,
            /*haunted*/  0xFCA3C0, 0xFCA3C0 + 0x2D9A,
            /*desert*/   0xFCD5C0, 0xFCD5C0 + 0x5121,
            /*beach*/    0xFD2CC0, 0xFD2CC0 + 0x3291,
            /*jungle*/   0xFD6900, 0xFD6900 + 0x1E33,
            /*swamp*/    0xFD8930, 0xFD8930 + 0x5139,
            /*ship*/     0xFDDE80, 0xFDDE80 + 0x4BD6,
            /*snow*/     0xFE3060, 0xFE3060 + 0x5414,
            /*training*/ 0xFE8CA0, 0xFE8CA0 + 0x2538,
            /*intro*/    0xFEB540, 0xFEB540 + 0x1BDD,
            /*witch*/    0xFED780, 0xFED780 + 0x6557,
            /*battle*/   0xFF4A50, 0xFF4A50 + 0x56AD,
            /*tree*/     0xFFA830, 0xFFA830 + 0x414E,
            /*coshow*/   0xFFF090, 0xFFF090 + 0xE,
            0xFFF0B0
        ),
        version => {panic!("file offsets not specified for {:?}", version)},
    };

    //slice rom
    let compressed_overlays = file_offsets.windows(2)
        .map(|w| {compressed_rom[w[0]..w[1]].to_vec()});

    //decompress slices
    // println!("Decompressing overlays...");
    let mut uncompressed_overlays : Vec<Vec<u8>>= compressed_overlays.map(|ovrly|{
        rarezip::bk::unzip(&ovrly)
    }).collect();

    uncompressed_overlays.swap(6, 8);
    uncompressed_overlays.swap(7, 9);

    //reconstruct rom
    let mut out_file = std::fs::File::create(target_path).unwrap();
    out_file.write_all(&compressed_rom[..file_offsets[0]]).unwrap();

    let mut rom_len = file_offsets[0];

    for (_i, bytes) in uncompressed_overlays.chunks(2).enumerate(){
        println!("placing code {} of length {:8X} at 0x{:08X?}", _i, bytes[0].len(), rom_len);
        out_file.write_all(&bytes[0]).unwrap();
        println!("placing data {} of length {:8X} at 0x{:08X?}", _i, bytes[1].len(), rom_len + bytes[0].len());
        out_file.write_all(&bytes[1]).unwrap();
        rom_len = rom_len + bytes[0].len() + bytes[1].len();
    }
}
