use ruuvi_schema::{RuuviRaw, RuuviRawE1, RuuviRawV2};

#[derive(Debug)]
pub enum ParseError {
    TooShort,
    UnknownFormat(u8),
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
            Ok(RuuviRaw::E1(RuuviRawE1::new(
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
                None,
            )))
        }
        0x5 => {
            // Assume any other format here maps to V2
            if data.len() < 24 {
                return Err(ParseError::TooShort);
            }
            Ok(RuuviRaw::V2(RuuviRawV2::new(
                i16::from_be_bytes([data[1], data[2]]),
                u16::from_be_bytes([data[3], data[4]]),
                u16::from_be_bytes([data[5], data[6]]),
                i16::from_be_bytes([data[7], data[8]]),
                i16::from_be_bytes([data[9], data[10]]),
                i16::from_be_bytes([data[11], data[12]]),
                u16::from_be_bytes([data[13], data[14]]),
                data[15],
                u16::from_be_bytes([data[16], data[17]]),
                [data[18], data[19], data[20], data[21], data[22], data[23]],
                None,
            )))
        }
        _ => Err(ParseError::UnknownFormat(data_format)),
    }
}
