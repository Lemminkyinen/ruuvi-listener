use serde::Serialize;

#[derive(Debug)]
pub enum ParseError {
    TooShort,
    UnknownFormat(u8),
}

#[derive(Debug, Clone, Serialize)]
pub struct RuuviRawV2 {
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

#[derive(Debug, Clone, Serialize)]
pub struct RuuviRawE1 {
    pub temp: i16,            // 1-2 raw, 0.005 °C units
    pub humidity: u16,        // 3-4 raw, 0.0025 % units
    pub pressure: u16,        // 5-6 raw, Pa with -50000 offset
    pub pm1_0: u16,           // 7-8 raw, 0.1 µg/m³
    pub pm2_5: u16,           // 9-10 raw, 0.1 µg/m³
    pub pm4_0: u16,           // 11-12 raw, 0.1 µg/m³
    pub pm10_0: u16,          // 13-14 raw, 0.1 µg/m³
    pub co2: u16,             // 15-16 raw, ppm
    pub voc_index: u16,       // 9-bit (byte17 << 1 | flags bit6)
    pub nox_index: u16,       // 9-bit (byte18 << 1 | flags bit7)
    pub luminosity: u32,      // 19-21 24-bit, 0.01 lux units
    pub measurement_seq: u32, // 25-27 24-bit counter
    pub flags: u8,            // 28
    pub mac: [u8; 6],         // 34-39
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub enum RuuviRaw {
    V2(RuuviRawV2),
    E1(RuuviRawE1),
}

impl RuuviRaw {
    pub fn measurement_seq(&self) -> u32 {
        match self {
            Self::E1(e1) => e1.measurement_seq,
            Self::V2(v2) => v2.measurement_seq as u32,
        }
    }

    pub fn mac(&self) -> [u8; 6] {
        match self {
            Self::E1(e1) => e1.mac,
            Self::V2(v2) => v2.mac,
        }
    }

    pub fn set_timestamp(&mut self, timestamp: Option<u64>) {
        match self {
            Self::E1(e1) => e1.timestamp = timestamp,
            Self::V2(v2) => v2.timestamp = timestamp,
        }
    }
}

pub fn parse_ruuvi_raw(data_format: u8, data: &[u8]) -> Result<RuuviRaw, ParseError> {
    match data_format {
        0xE1 => {
            if data.len() < 40 {
                return Err(ParseError::TooShort);
            }
            let temp = i16::from_be_bytes([data[1], data[2]]);
            let humidity = u16::from_be_bytes([data[3], data[4]]);
            let pressure = u16::from_be_bytes([data[5], data[6]]);
            let pm1_0 = u16::from_be_bytes([data[7], data[8]]);
            let pm2_5 = u16::from_be_bytes([data[9], data[10]]);
            let pm4_0 = u16::from_be_bytes([data[11], data[12]]);
            let pm10_0 = u16::from_be_bytes([data[13], data[14]]);
            let co2 = u16::from_be_bytes([data[15], data[16]]);
            let flags = data[28];

            // https://docs.ruuvi.com/communication/bluetooth-advertisements/data-format-e1
            // Check later
            let voc_index = ((data[17] as u16) << 1) | ((flags >> 6) & 0x01) as u16;
            let nox_index = ((data[18] as u16) << 1) | ((flags >> 7) & 0x01) as u16;
            let luminosity =
                ((data[19] as u32) << 16) | ((data[20] as u32) << 8) | (data[21] as u32);
            let measurement_seq =
                ((data[25] as u32) << 16) | ((data[26] as u32) << 8) | (data[27] as u32);
            let mac = [data[34], data[35], data[36], data[37], data[38], data[39]];
            Ok(RuuviRaw::E1(RuuviRawE1 {
                temp,
                humidity,
                pressure,
                pm1_0,
                pm2_5,
                pm4_0,
                pm10_0,
                co2,
                voc_index,
                nox_index,
                luminosity,
                measurement_seq,
                flags,
                mac,
                timestamp: None,
            }))
        }
        0x5 => {
            // Assume any other format here maps to V2
            if data.len() < 24 {
                return Err(ParseError::TooShort);
            }
            Ok(RuuviRaw::V2(RuuviRawV2 {
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
            }))
        }
        _ => Err(ParseError::UnknownFormat(data_format)),
    }
}
