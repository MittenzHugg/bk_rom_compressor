use std::env;
use std::fs::{self};
use std::io::{Write};
use rarezip;
use elf;

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
struct Config{
    comp_rom_path: String,
    uncomp_rom_path: String,
    elf_path: String,
    game_id: GameId, 
}


//compress [-v pal] bk.elf bk.uncompressed.z64 bk.compressed.z64
impl Config{
    fn form_args(args : &mut env::Args) -> Self{
        let help_text = include_str!("comp_help.txt");

        let mut config = Config{
            game_id : GameId::BanjoKazooie(GameVersion::USA),
            elf_path : String::new(),
            uncomp_rom_path : String::new(),
            comp_rom_path : String::new(),
        };

        config.comp_rom_path = args.next_back().expect(format!("No output path specified\n\n{}", help_text).as_str());
        config.uncomp_rom_path = args.next_back().expect(format!("No input ROM path specified\n\n{}", help_text).as_str());
        config.elf_path = args.next_back().expect(format!("No input ELF path specified\n\n{}", help_text).as_str());
        let mut set_version : bool = false;
        for a in args.skip(1) {
            if set_version {
                config.game_id = match a.as_str() {
                    "us.v10" => GameId::BanjoKazooie(GameVersion::USA),
                    "pal"    => GameId::BanjoKazooie(GameVersion::PAL),
                    "jp"     => GameId::BanjoKazooie(GameVersion::JP),
                    "us.v11" => GameId::BanjoKazooie(GameVersion::USARevA),
                    _ => panic!("Unknown version\n\n{}", help_text),
                };
                set_version = false;
            } else {
                match a.as_str() {
                    "-v" | "--version" => {set_version = true;},
                    _ => panic!("Unknown option\n\n{}", help_text),
                }
            }
        }
        return config
    }
}


#[derive(Debug, Clone)]
struct OverlayInfo {
    name: String,
    text: std::ops::Range<usize>,
    data: std::ops::Range<usize>,
}

impl OverlayInfo {
    fn from_elf_symbols(name: &str, symbols: &[elf::types::Symbol]) -> Self{
        OverlayInfo{
            name: String::from(name),
            text: std::ops::Range{
                start: match symbols.into_iter().find(|s| {s.name == format!("{}_TEXT_START", name)}){
                    Some(sym) => sym.value as usize,
                    None => panic!("could not find symbol {}_TEXT_START in elf", name),
                }, 
                end: match symbols.into_iter().find(|s| {s.name == format!("{}_TEXT_END", name)}){
                    Some(sym) => sym.value as usize,
                    None => panic!("could not find symbol {}_TEXT_END in elf", name),
                }, 
            },
            data: std::ops::Range{
                start: match symbols.into_iter().find(|s| {s.name == format!("{}_DATA_START", name)}){
                    Some(sym) => sym.value as usize,
                    None => panic!("could not find symbol {}_DATA_START in elf", name),
                }, 
                end: match symbols.into_iter().find(|s| {s.name == format!("{}_DATA_END", name)}){
                    Some(sym) => sym.value as usize,
                    None => panic!("could not find symbol {}_DATA_END in elf", name),
                }, 
            }
        }
    }

    fn len(&self)->usize{
        self.text.len() + self.data.len()
    }
}


fn bk_crc(bytes : &[u8]) -> (u32, u32){
    let crc : (u32, u32) = (0, 0xFFFFFFFF);
    bytes.iter().fold(crc, |crc, byte| {
        let a = crc.0 + (*byte as u32); 
        let b = crc.1 ^ ((*byte as u32) << (crc.0 & 0x17));
        return (a, b)
    })
}

fn main() {
    //parse command line args
    let config = Config::form_args(&mut env::args());

    //check input file exists
    assert!(fs::metadata(&config.uncomp_rom_path).unwrap().is_file(), "Input \"{}\" not found", config.uncomp_rom_path);
    assert!(fs::metadata(&config.elf_path).unwrap().is_file(), "Elf \"{}\" not found", config.elf_path);
    let uncompressed_rom : Vec<u8> = fs::read(&config.uncomp_rom_path).expect("Could not read uncompressed rom file");

    
    let rom_overlay_start : usize = match config.game_id {
        GameId::BanjoKazooie(GameVersion::USA) => {0xF19250},
        version => {panic!("file offsets not specified for {:?}", version)},
    };

    let mut elf_file = match elf::File::open_path(&config.elf_path) {
        Ok(f) => f,
        Err(e) => panic!("{:?}",e),
    };
 
    //grab all symbols in elf
    println!("Finding section symbols...");
    let symbols : Vec<elf::types::Symbol> = elf_file.sections.iter().map(|section| {
        match elf_file.get_symbols(&section) {
            Ok(s) => s,
            Err(e) => panic!("{:?}",e),
        }
    }).flatten().collect();

    //TODO: get all overlays
    // let overlay_names = vec!["core1, core2, CC, GV, MMM, TTC, MM, BGS, RBB, FP, SM, cutscenes, lair, fight, CCW"];
    let overlay_names = vec!["core1"];
    let overlay_ram_info = overlay_names.iter().clone().map(|ovrly_name| {OverlayInfo::from_elf_symbols(ovrly_name, &symbols)});
    let mut i_offset = rom_overlay_start;
    let overlay_uncomp_rom_offsets : Vec<OverlayInfo> = overlay_ram_info.clone().map(|mut x|{
        let diff = x.text.start - i_offset;
        x.text.start = x.text.start  - diff;
        x.text.end = x.text.end  - diff;
        x.data.start = x.data.start  - diff;
        x.data.end   = x.data.end  - diff;
        i_offset = i_offset + x.len();
        x
    }).collect();

    let uncomp_code_bytes = overlay_uncomp_rom_offsets.iter().map(|x| {
        uncompressed_rom[x.text.start .. x.text.end].to_vec()
    });
    let uncomp_data_bytes = overlay_uncomp_rom_offsets.iter().map(|x| {
        uncompressed_rom[x.data.start .. x.data.end].to_vec()
    });

    
    println!("Calculating CRCs...");
    let code_crcs = uncomp_code_bytes.clone().map(|c_bytes| { bk_crc(&c_bytes) });
    for (name, crc) in overlay_names.iter().zip(code_crcs){
        println!("{} (0x{:08X?}, 0x{:08X?})", name, crc.0, crc.1);
    }
    // TODO!!! : replaces code crc symbols
    // TODO!!! : calculate data crcs
    // TODO!!! : replace data crc symbols

    println!("Compressing Overlays...");
    let comp_code_bytes : Vec<Vec<u8>> = uncomp_code_bytes.clone().map(|uncomp| {rarezip::bk::zip(&uncomp)}).collect();
    let comp_data_bytes : Vec<Vec<u8>> = uncomp_data_bytes.clone().map(|uncomp| {rarezip::bk::zip(&uncomp)}).collect();

    for (name, (code, data)) in overlay_names.iter().zip(comp_code_bytes.iter().zip(comp_data_bytes)){
        println!("{}: code: 0x{:08X?} bytes, data: 0x{:08X?} bytes)", name, code.len(), data.len());
    }
    //create output
    println!("Creating ROM {} => {}", config.uncomp_rom_path, config.comp_rom_path);
    let mut out_file = std::fs::File::create(config.comp_rom_path).unwrap();
    out_file.write_all(&uncompressed_rom[..rom_overlay_start]).unwrap();
    //read in binary and convert to big endian
    // let compressed_rom : Vec<u8> = fs::read(source_path).expect("Could not read file \"{}\"");

}
