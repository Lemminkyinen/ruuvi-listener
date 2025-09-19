use serde::Deserialize;

#[repr(C)]
#[derive(Debug, Clone, Copy, Deserialize)]
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
