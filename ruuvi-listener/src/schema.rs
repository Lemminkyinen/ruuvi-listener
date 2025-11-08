use serde::Serialize;

fn as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe { core::slice::from_raw_parts((p as *const T) as *const u8, core::mem::size_of::<T>()) }
}

#[derive(Debug)]
pub enum ParseError {
    TooShort,
    UnknownFormat(u8),
}

#[repr(C)]
#[derive(Debug, Clone, Serialize)]
pub struct RuuviRawV2 {
    pub format: u8,             // 0
    pub temp: i16,              // 1-2
    pub humidity: u16,          // 3-4
    pub pressure: u16,          // 5-6
    pub acc_x: i16,             // 7-8
    pub acc_y: i16,             // 9-10
    pub acc_z: i16,             // 11-12
    pub power_info: u16,        // 13-14
    pub movement_counter: u8,   // 15
    pub measurement_seq: u16,   // 16-17
    pub mac: [u8; 6],           // 18-23
    pub timestamp: Option<u64>, // Added field
}

#[repr(C)]
#[derive(Debug, Clone, Serialize)]
pub struct RuuviRawV6 {
    pub format: u8,          // 0 (should be 0x06)
    pub temp: i16,           // 1-2, 0.005 °C units
    pub humidity: u16,       // 3-4, 0.0025 % units
    pub pressure: u16,       // 5-6, Pa with -50000 offset
    pub pm2_5: u16,          // 7-8, 0.1 µg/m³
    pub co2: u16,            // 9-10, ppm
    pub voc_index: u16,      // 11 + flags b6, 9 bits
    pub nox_index: u16,      // 12 + flags b7, 9 bits
    pub luminosity: u8,      // 13, logarithmic
    pub reserved: u8,        // 14, always 255
    pub measurement_seq: u8, // 15
    pub flags: u8,           // 16
    pub half_mac: [u8; 3],   // 17-19, lowest 3 bytes of MAC
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub enum RuuviRaw {
    V2(RuuviRawV2),
    E1(RuuviRawV6),
}

impl RuuviRaw {
    pub fn measurement_seq(&self) -> u16 {
        match self {
            Self::E1(e1) => e1.measurement_seq as u16,
            Self::V2(v2) => v2.measurement_seq,
        }
    }

    pub fn mac(&self) -> [u8; 6] {
        match self {
            Self::E1(e1) => [e1.half_mac[0], e1.half_mac[1], e1.half_mac[2], 0, 0, 0],
            Self::V2(v2) => v2.mac,
        }
    }

    // pub fn get_timestamp(&self) -> Option<u64> {
    //     match self {
    //         Self::E1(e1) => e1.timestamp,
    //         Self::V2(v2) => v2.timestamp,
    //     }
    // }

    pub fn set_timestamp(&mut self, timestamp: Option<u64>) {
        match self {
            Self::E1(e1) => e1.timestamp = timestamp,
            Self::V2(v2) => v2.timestamp = timestamp,
        }
    }

    pub fn to_bytes(&self) -> &[u8] {
        match self {
            Self::E1(e1) => as_u8_slice(e1),
            Self::V2(v2) => as_u8_slice(v2),
        }
    }
}

pub fn parse_ruuvi_raw(data: &[u8]) -> Result<RuuviRaw, ParseError> {
    let Some(format) = data.first() else {
        return Err(ParseError::TooShort);
    };

    if *format == 0x6 {
        if data.len() < 20 {
            return Err(ParseError::TooShort);
        }
        let temp = i16::from_be_bytes([data[1], data[2]]);
        let humidity = u16::from_be_bytes([data[3], data[4]]);
        let pressure = u16::from_be_bytes([data[5], data[6]]);
        let pm2_5 = u16::from_be_bytes([data[7], data[8]]);
        let co2 = u16::from_be_bytes([data[9], data[10]]);
        let voc_index = ((data[11] as u16) << 1) | ((data[16] >> 6) & 0x01) as u16;
        let nox_index = ((data[12] as u16) << 1) | ((data[16] >> 7) & 0x01) as u16;
        let luminosity = data[13];
        let reserved = data[14];
        let measurement_seq = data[15];
        let flags = data[16];
        let mac = [data[17], data[18], data[19]];
        return Ok(RuuviRaw::E1(RuuviRawV6 {
            format: *format,
            temp,
            humidity,
            pressure,
            pm2_5,
            co2,
            voc_index,
            nox_index,
            luminosity,
            reserved,
            measurement_seq,
            flags,
            half_mac: mac,
            timestamp: None,
        }));
    }

    if *format != 0x5 {
        return Err(ParseError::UnknownFormat(*format));
    }

    // Assume any other format here maps to V2
    if data.len() < 24 {
        return Err(ParseError::TooShort);
    }
    let raw = RuuviRawV2 {
        format: *format,
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
        timestamp: None,
    };
    Ok(RuuviRaw::V2(raw))
}
