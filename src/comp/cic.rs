const HEADER_SIZE: usize = 0x40;
const BC_SIZE: usize = 0x1000 - HEADER_SIZE;
const POLYNOMIAL: u32 = 0xedb88320;

const CRC_TABLE : [u32; 256] = crc_generate_table();

const fn crc_generate_table()->[u32; 256] {
    let mut table = [0;256];
    let mut i: usize = 0;
    while i < 256 {
        let mut bit : usize = 8;
        let mut crc = i as u32;
        while bit > 0 {
            crc = if (crc & 1) != 0 {
                (crc >> 1) ^ POLYNOMIAL
            } else {
                crc >> 1
            };
            bit = bit -1;
        }
        table[i] = crc;
        i = i + 1;
    }
    return table;
}

#[derive(PartialEq, Debug)]
pub enum N64CicType {
    Cic6101,
    Cic6102,
    Cic6103,
    Cic6105,
    Cic6106,
}

fn crc32(data: &[u8])-> u32 {
    let mut crc = 0xFFFFFFFF;
    for byte in data {
        crc = (crc >> 8) ^ CRC_TABLE[((crc as u8) ^ byte) as usize];
    }
    return !crc;
}

pub fn identify(rom : &[u8])->Option<N64CicType> {
    return match crc32(&rom[HEADER_SIZE .. HEADER_SIZE + BC_SIZE]) {
        0x6170a4a1 => Some(N64CicType::Cic6101),
        0x90bb6cb5 => Some(N64CicType::Cic6102),
        0x0B050ee0 => Some(N64CicType::Cic6103),
        0x98bc2c86 => Some(N64CicType::Cic6105),
        0xacc8580a => Some(N64CicType::Cic6106),
        _ => None,
    }
}

#[allow(arithmetic_overflow)]
pub fn calculate_crc(rom : &[u8]) -> Option<[u32; 2]> {
    let bootcode = identify(rom)?;
    let seed : u32 = match bootcode {
        N64CicType::Cic6101  | N64CicType::Cic6102 => 0xF8CA4DDC,
        N64CicType::Cic6103 => 0xA3886759,
        N64CicType::Cic6105 => 0xDF26F436,
        N64CicType::Cic6106 => 0x1FEA617A,
    };
    let mut t1 = seed;
    let mut t2 = seed;
    let mut t3 = seed;
    let mut t4 = seed;
    let mut t5 = seed;
    let mut t6 = seed;

    let crc_section = &rom[0x1000 .. 0x1000 + 0x100000];
    let words = crc_section.chunks_exact(4).map(|bytes| u32::from_be_bytes(bytes.try_into().unwrap()));
    for (i, d) in words.enumerate() {
        t4 = t4.wrapping_add(if t6.wrapping_add(d) < t6 {1} else {0}); 
        t6 = t6.wrapping_add(d);
		t3 = t3 ^ d;
        let r = (d.checked_shl(d & 0x1F).unwrap_or(0)) | (d.checked_shr(32 - (d & 0x1F)).unwrap_or(0));
		t5 = t5.wrapping_add(r);
        t2 = t2 ^ (if t2 > d { r } else { t6 ^ d });
        t1 = t1.wrapping_add(d ^ (if bootcode == N64CicType::Cic6105{
            let offset = (4*i + 0x710) & 0xff;
            u32::from_be_bytes(crc_section[offset .. offset + 4].try_into().unwrap())
        } else {
            t5
        }));
    }
    return Some(match bootcode {
            N64CicType::Cic6103 => [(t6 ^ t4) + t3 , (t5 ^ t2) + t1],
            N64CicType::Cic6106 => [(t6 * t4) + t3 , (t5 * t2) + t1],
            _ => [t6 ^ t4 ^ t3 , t5 ^ t2 ^ t1],
    })
}