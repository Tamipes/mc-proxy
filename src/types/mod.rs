use std::fmt::Display;

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

#[derive(Debug)]
pub struct VarInt {
    value: i32,
    data: Vec<u8>,
}

impl Display for VarInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl VarInt {
    pub fn get_int(&self) -> i32 {
        self.value
    }

    /// Clones the data for use, the sturct is still usable.
    pub fn get_data(&self) -> Vec<u8> {
        self.data.clone()
    }
    /// Moves the data out from the struct. Struct is useless later.
    pub fn move_data(self) -> Vec<u8> {
        self.data
    }
    pub fn read<I>(data: &mut I) -> Option<i32>
    where
        I: Iterator<Item = u8>,
    {
        let mut value: i32 = 0;
        let mut position = 0;

        for current_byte in data {
            value |= ((current_byte & SEGMENT_BITS) as i32) << position;

            if current_byte & CONTINUE_BIT == 0 {
                break;
            }
            position += 7;

            if position > 32 {
                todo!();
            }
        }
        Some(value)
    }
    pub fn parse<I>(reader: &mut I) -> Option<VarInt>
    where
        I: Iterator<Item = u8>,
    {
        let mut value: i32 = 0;
        let mut position = 0;
        let mut vec = Vec::new();

        for current_byte in reader {
            let current_byte = current_byte;
            vec.push(current_byte);
            value |= ((current_byte & SEGMENT_BITS) as i32) << position;

            if current_byte & CONTINUE_BIT == 0 {
                break;
            }
            position += 7;

            if position > 32 {
                return None;
            }
        }
        Some(VarInt { value, data: vec })
    }
    pub fn from(num: i32) -> Option<VarInt> {
        Some(VarInt {
            value: num,
            data: VarInt::write_varint(num)?,
        })
    }
    fn write_varint(num: i32) -> Option<Vec<u8>> {
        let mut num = num;
        let mut vec = Vec::new();
        if num == 0 {
            vec.push(0);
        }
        while num != 0 {
            vec.push(num as u8 & SEGMENT_BITS);
            num = num >> 7;
            if num != 0 {
                let a = vec.pop()?;
                vec.push(a | CONTINUE_BIT);
            }
        }
        Some(vec)
    }
}

#[derive(Debug)]
pub struct VarString {
    value: String,
}

impl VarString {
    pub fn get_value(&self) -> String {
        self.value.clone()
    }
    pub fn move_data(self) -> Option<Vec<u8>> {
        let mut vec = VarInt::from(self.value.len() as i32)?.move_data();
        vec.append(&mut (Vec::from(self.value.as_bytes())));
        Some(vec)
    }

    pub fn get_data(&self) -> Option<Vec<u8>> {
        let mut vec = VarInt::from(self.value.len() as i32)?.move_data();
        vec.append(&mut (Vec::from(self.value.as_bytes())));
        Some(vec)
    }

    pub fn from(string: String) -> VarString {
        VarString { value: string }
    }
    pub fn parse<I>(data: &mut I) -> Option<VarString>
    where
        I: Iterator<Item = u8>,
    {
        let length = VarInt::read(data)?;
        let mut vec = Vec::new();
        for _ in 0..length {
            vec.push(data.next()?);
        }
        Some(VarString {
            value: String::from_utf8(vec).ok()?,
        })
    }
}

pub struct UShort {
    value: u16,
    data: Vec<u8>,
}
impl UShort {
    pub fn get_value(&self) -> u16 {
        self.value
    }
    pub fn get_data(&self) -> Vec<u8> {
        self.data.clone()
    }
    pub fn parse<I>(data: &mut I) -> Option<UShort>
    where
        I: Iterator<Item = u8>,
    {
        let mut vec = vec![data.next()?];
        let mut int: u16 = vec[0] as u16;
        int = int << 8;
        vec.push(data.next()?);
        int |= vec[1] as u16;
        Some(UShort {
            value: int,
            data: vec,
        })
    }
    pub fn from(short: u16) -> UShort {
        let mut vec = vec![(short >> 8) as u8];
        vec.push(((short >> 8) << 8) as u8);
        UShort {
            value: short,
            data: vec,
        }
    }
}
