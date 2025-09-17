#[derive(Debug)]
pub enum ParseError {
    TooShort,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RuuviRawV2 {
    pub format: u8,           // 0
    pub temp: i16,            // 1-2
    pub humidity: u16,        // 3-4
    pub pressure: u16,        // 5-6
    pub acc_x: i16,           // 7-8
    pub acc_y: i16,           // 9-10
    pub acc_z: i16,           // 11-12
    pub power_info: u16,      // 13-14
    pub movement_counter: u8, // 15
    pub measurement_seq: u16, // 16-17
    pub mac: [u8; 6],         // 18-23
}

impl RuuviRawV2 {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() < 24 {
            return Err(ParseError::TooShort);
        }
        Ok(Self {
            format: data[0],
            temp: i16::from_be_bytes([data[1], data[2]]),
            humidity: u16::from_be_bytes([data[3], data[4]]),
            pressure: u16::from_be_bytes([data[5], data[6]]),
            acc_x: i16::from_be_bytes([data[7], data[8]]),
            acc_y: i16::from_be_bytes([data[9], data[10]]),
            acc_z: i16::from_be_bytes([data[11], data[12]]),
            power_info: u16::from_be_bytes([data[13], data[14]]),
            movement_counter: data[15],
            measurement_seq: u16::from_be_bytes([data[16], data[17]]),
            mac: [data[18], data[19], data[20], data[21], data[22], data[23]],
        })
    }
}
