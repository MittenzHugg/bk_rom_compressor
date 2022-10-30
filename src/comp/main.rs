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

fn find_elf_symbol(symbols: &[elf::types::Symbol], name: &str)->elf::types::Symbol{
    return match symbols.into_iter().find(|s| {s.name == name}){
        Some(sym) => sym.clone(),
        None => panic!("could not find symbol {} in elf symbols", name),
    }
}

#[derive(Debug, Clone)]
struct OverlayInfo {
    name: String,
    text: std::ops::Range<usize>,
    data: std::ops::Range<usize>,
    bss:  std::ops::Range<usize>,
    uncompressed_rom: std::ops::Range<usize>,
}

impl OverlayInfo {
    fn from_elf_symbols(name: &str, symbols: &[elf::types::Symbol]) -> Self{
        OverlayInfo{
            name: String::from(name),
            text: std::ops::Range{
                start:  find_elf_symbol(symbols, format!("{}_TEXT_START", name).as_str()).value as usize,
                // end:    find_elf_symbol(symbols, format!("{}_TEXT_END", name).as_str()).value as usize,
                end:  find_elf_symbol(symbols, match name {
                    "core1" => format!("{}_DATA_START_OFFSET", name),
                    _ => format!("{}_TEXT_END", name),
                }.as_str()).value as usize,
            },
            data: std::ops::Range{
                start:  find_elf_symbol(symbols, match name {
                    "core1" => format!("{}_DATA_START_OFFSET", name),
                    _ => format!("{}_DATA_START", name),
                }.as_str()).value as usize,
                end:    find_elf_symbol(symbols, format!("{}_DATA_END", name).as_str()).value as usize,
            },
            bss: std::ops::Range{
                start:  find_elf_symbol(symbols, format!("{}_BSS_START", name).as_str()).value as usize,
                end:    find_elf_symbol(symbols, format!("{}_BSS_END", name).as_str()).value as usize,
            },
            uncompressed_rom: std::ops::Range{
                start:  find_elf_symbol(symbols, format!("{}_ROM_START", name).as_str()).value as usize,
                end:    find_elf_symbol(symbols, format!("{}_ROM_END", name).as_str()).value as usize, 
            },
        }
    }

    fn len(&self)->usize{
        return self.text.len() + self.data.len();
    }
}


fn bk_crc(bytes : &[u8]) -> (u32, u32){
    let crc : (u32, u32) = (0, 0xFFFFFFFF);
    bytes.iter().fold(crc, |crc, byte| {
        let a = crc.0 + (*byte as u32); 
        let b = crc.1 ^ ((*byte as u32) << (a & 0x17));
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

    let elf_file = match elf::File::open_path(&config.elf_path) {
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
    // symbols.iter().for_each(|s| {println!("{} @ 0x{:08X}", s.name, s.value);});

    let bk_boot_info = OverlayInfo::from_elf_symbols("boot_bk_boot", &symbols);
    let mut bk_boot_bytes = uncompressed_rom[bk_boot_info.uncompressed_rom.clone()].to_vec();
    // println!{"{:#08X?}", bk_boot_info};

    //overlays offsets from elf symbols
    let mut overlay_names = vec!["core1", "core2", "CC", "GV", "MMM", "TTC", "MM", "BGS", "RBB", "FP", "SM", "cutscenes", "lair", "fight", "CCW"];
    let overlay_offsets : Vec<OverlayInfo> = overlay_names.iter().clone().map(|ovrly_name| {OverlayInfo::from_elf_symbols(ovrly_name, &symbols)}).collect();
    // overlay_offsets.iter().for_each(|info| {println!{"{:#08X?}", info}});

    //seperate bits
    let uncomp_code_bytes = overlay_offsets.iter().map(|x| {
        uncompressed_rom[x.uncompressed_rom.start .. x.uncompressed_rom.start + x.text.len()].to_vec()
    });
    
    let mut uncomp_data_bytes : Vec<Vec<u8>>= overlay_offsets.iter().map(|x| {
        uncompressed_rom[x.uncompressed_rom.start + x.text.len() .. x.uncompressed_rom.end].to_vec()
    }).collect();

    println!("Calculating Overlay CRCs...");
    let code_crcs :Vec<_>= uncomp_code_bytes.clone().map(|c_bytes| { bk_crc(&c_bytes) }).collect();
    for (name, crc) in overlay_names.iter().zip(&code_crcs){
        println!("{} (0x{:08X?}, 0x{:08X?})", name, crc.0, crc.1);
    }


    let replace_symbol = |bytes: &mut Vec<u8>, rom_offset: usize, symbol_name : &str, value : [u8; 4]|{
        let s = find_elf_symbol(&symbols, symbol_name);
        let offset = s.value as usize - rom_offset;
        bytes.splice(offset .. offset+value.len(), value);
    };

    //Replace Overlay CRC's
    let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "SM"}).unwrap().0;
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038AAE0", code_crcs[indx].0.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038AAE4", code_crcs[indx].1.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038AAE8", [0;4]);
    let data_crc = bk_crc(&uncomp_data_bytes[indx]);
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038AAE8", data_crc.0.to_be_bytes());

    let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "MM"}).unwrap().0;
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_803899C0", code_crcs[indx].0.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_803899C4", code_crcs[indx].1.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_803899C8", [0;4]);
    let data_crc = bk_crc(&uncomp_data_bytes[indx]);
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_803899C8", data_crc.0.to_be_bytes());

    let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "TTC"}).unwrap().0;
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038C750", code_crcs[indx].0.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038C754", code_crcs[indx].1.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038C758", [0;4]);
    let data_crc = bk_crc(&uncomp_data_bytes[indx]);
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038C758", data_crc.0.to_be_bytes());

    let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "BGS"}).unwrap().0;
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80390B20", code_crcs[indx].0.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80390B24", code_crcs[indx].1.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80390B28", [0;4]);
    let data_crc = bk_crc(&uncomp_data_bytes[indx]);
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80390B28", data_crc.0.to_be_bytes());

    let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "CC"}).unwrap().0;
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80389BE0", code_crcs[indx].0.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80389BE4", code_crcs[indx].1.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80389BE8", [0;4]);
    let data_crc = bk_crc(&uncomp_data_bytes[indx]);
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80389BE8", data_crc.0.to_be_bytes());

    let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "GV"}).unwrap().0;
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80390F30", code_crcs[indx].0.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80390F34", code_crcs[indx].1.to_be_bytes());
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80390F38", [0;4]);
    let data_crc = bk_crc(&uncomp_data_bytes[indx]);
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80390F38", data_crc.0.to_be_bytes());

    // let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "MMM"}).unwrap().0;
    // replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038C300", code_crcs[indx].0.to_be_bytes());
    // replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038C304", code_crcs[indx].1.to_be_bytes());
    // replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038C308", [0;4]);
    // let data_crc = bk_crc(&uncomp_data_bytes[indx]);
    // replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_8038C308", data_crc.0.to_be_bytes());

    let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "core2"}).unwrap().0;
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_803727F4", code_crcs[indx].1.to_be_bytes());
    let core2_data_crc = bk_crc(&uncomp_data_bytes[indx]);

    let indx = overlay_names.clone().into_iter().enumerate().find(|(_, name)| {*name == "core1"}).unwrap().0;
    replace_symbol(&mut uncomp_data_bytes[indx], overlay_offsets[indx].data.start, "D_80276574", core2_data_crc.1.to_be_bytes());
    let core1_data_crc = bk_crc(&uncomp_data_bytes[indx]);
    let core1_code_crc = code_crcs[indx];

    let crc_rom_start = find_elf_symbol(&symbols, "crc_ROM_START");
    let mut rom_crc_bytes: Vec<u8> = vec![0; 0x20];
    rom_crc_bytes.splice(8..0xC, core1_code_crc.0.to_be_bytes());
    rom_crc_bytes.splice(0xC..0x10, core1_code_crc.1.to_be_bytes());
    rom_crc_bytes.splice(0x10..0x14, core1_data_crc.0.to_be_bytes());
    rom_crc_bytes.splice(0x14..0x18, core1_data_crc.1.to_be_bytes());

    println!("Compressing Overlays...");
    let mut rzip_bytes : Vec<Vec<u8>> = uncomp_code_bytes.zip(uncomp_data_bytes).map(|(code, data)| {
        let mut code_rzip = rarezip::bk::zip(&code);
        let mut data_rzip = rarezip::bk::zip(&data);
        code_rzip.append(&mut data_rzip);
        code_rzip.resize(code_rzip.len() + (16-1) & !(16-1), 0);
        return code_rzip
    }).collect();

    println!("TODO!!! update bk_boot hardcoded compressed rom file offsets...\n");

    overlay_names.swap(3, 4);
    rzip_bytes.swap(3, 4);
    let overlay_start_offset = overlay_offsets[0].uncompressed_rom.start;
    let mut i_offset = overlay_start_offset;
    for (name, rzip) in overlay_names.iter().zip(rzip_bytes.iter()){
        println!("-D{}_us_v10_rzip_ROM_START=0x{:08X?}", name, i_offset);
        println!("-D{}_us_v10_rzip_ROM_END=0x{:08X?}", name, i_offset + rzip.len());
        i_offset = i_offset + rzip.len();
    }
    println!("\nTODO!!! updata bk_boot hardcoded compressed file offsets...");

    println!("Calculating ROM CRCs...");
    let bk_boot_crc = bk_crc(&bk_boot_bytes);
    let crc_rom_start = find_elf_symbol(&symbols, "crc_ROM_START").value as usize;
    let mut rom_crc_bytes: Vec<u8> = vec![0; 0x20];
    rom_crc_bytes.splice(0..4, bk_boot_crc.0.to_be_bytes());
    rom_crc_bytes.splice(4..8, bk_boot_crc.1.to_be_bytes());
    rom_crc_bytes.splice(8..0xC, core1_code_crc.0.to_be_bytes());
    rom_crc_bytes.splice(0xC..0x10, core1_code_crc.1.to_be_bytes());
    rom_crc_bytes.splice(0x10..0x14, core1_data_crc.0.to_be_bytes());
    rom_crc_bytes.splice(0x14..0x18, core1_data_crc.1.to_be_bytes());


    //todo update bk_boot

//     //create output
    println!("Creating ROM {} => {}", config.uncomp_rom_path, config.comp_rom_path);
    let mut out_file = std::fs::File::create(config.comp_rom_path).unwrap();
    out_file.write_all(&uncompressed_rom[..bk_boot_info.uncompressed_rom.start]).unwrap();
    out_file.write_all(&bk_boot_bytes).unwrap();
    out_file.write_all(&rom_crc_bytes).unwrap();
    out_file.write_all(&uncompressed_rom[crc_rom_start as usize + 0x20 .. overlay_start_offset]).unwrap();
    for rzip_bin in rzip_bytes{
        out_file.write_all(&rzip_bin).unwrap();
    }
    let mut empty_lvl = rarezip::bk::zip(&[0x03,0xE0, 0x00, 0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    empty_lvl.append(&mut rarezip::bk::zip(&[0x0;0x10]));
    empty_lvl.resize(empty_lvl.len() + (16-1) & !(16-1), 0);
    out_file.write_all(&empty_lvl).unwrap();

    out_file.write_all(&vec![0xFF; 0x1000000 - (i_offset + 0x20)]).unwrap();
    
    // write rest;
    //read in binary and convert to big endian
    // let compressed_rom : Vec<u8> = fs::read(source_path).expect("Could not read file \"{}\"");


}
