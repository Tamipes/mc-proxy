const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

pub fn read_varint<I>(data: &mut I) -> Option<i32>
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

pub fn write_string(string: String) -> Vec<u8> {
    let mut vec = write_varint(string.len() as i32);
    vec.append(&mut (Vec::from(string.as_bytes())));
    vec
}

pub fn write_varint(num: i32) -> Vec<u8> {
    let mut num = num;
    let mut vec = Vec::new();
    if num == 0 {
        vec.push(0);
    }
    while num != 0 {
        vec.push(num as u8 & SEGMENT_BITS);
        num = num >> 7;
        if num != 0 {
            let a = vec.pop().unwrap();
            vec.push(a | CONTINUE_BIT);
        }
    }
    vec
}

pub fn read_varint_data<I>(reader: &mut I) -> Option<(i32, Vec<u8>)>
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
    Some((value, vec))
}

pub fn read_string<I>(data: &mut I) -> Option<String>
where
    I: Iterator<Item = u8>,
{
    let length = read_varint(data).unwrap();
    let mut vec = Vec::new();
    for i in 0..length {
        vec.push(data.next().unwrap());
    }
    Some(String::from_utf8(vec).unwrap())
}

pub fn read_ushort<I>(data: &mut I) -> Option<u16>
where
    I: Iterator<Item = u8>,
{
    let mut int: u16 = data.next().unwrap() as u16;
    int = int << 8;
    int |= data.next().unwrap() as u16;
    Some(int)
}
