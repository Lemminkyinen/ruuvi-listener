#![cfg_attr(not(feature = "std"), no_std)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuuviRawV2 {
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
    // Added fields
    pub timestamp: Option<u64>,
    pub rssi: i8,
}

impl RuuviRawV2 {
    pub const fn new(
        temp: i16,
        humidity: u16,
        pressure: u16,
        acc_x: i16,
        acc_y: i16,
        acc_z: i16,
        power_info: u16,
        movement_counter: u8,
        measurement_seq: u16,
        mac: [u8; 6],
        timestamp: Option<u64>,
        rssi: i8,
    ) -> Self {
        Self {
            temp,
            humidity,
            pressure,
            acc_x,
            acc_y,
            acc_z,
            power_info,
            movement_counter,
            measurement_seq,
            mac,
            timestamp,
            rssi,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    // Added fields
    pub timestamp: Option<u64>,
    pub rssi: i8,
    pub tx_power: i8,
}

impl RuuviRawE1 {
    pub const fn new(
        temp: i16,
        humidity: u16,
        pressure: u16,
        pm1_0: u16,
        pm2_5: u16,
        pm4_0: u16,
        pm10_0: u16,
        co2: u16,
        voc_index: u16,
        nox_index: u16,
        luminosity: u32,
        measurement_seq: u32,
        flags: u8,
        mac: [u8; 6],
        timestamp: Option<u64>,
        rssi: i8,
        tx_power: i8,
    ) -> Self {
        Self {
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
            timestamp,
            rssi,
            tx_power,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
