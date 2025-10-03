#[no_std]
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

pub mod noise {
    use snow::{Builder, HandshakeState, params::NoiseParams};

    const PARAMS: &str = "Noise_XXpsk3_25519_ChaChaPoly_SHA-256";

    fn build_noise(secret: &'static [u8; 32], private_key: &'static [u8]) -> Builder<'static> {
        let params: NoiseParams = PARAMS.parse().unwrap();
        let builder = Builder::new(params);
        // let private_key = builder.generate_keypair().unwrap().private;
        builder
            .local_private_key(private_key)
            .unwrap()
            .psk(3, secret)
            .unwrap()
    }

    pub fn build_noise_responder(
        secret: &'static [u8; 32],
        private_key: &'static [u8],
    ) -> HandshakeState {
        build_noise(secret, private_key).build_responder().unwrap()
    }

    pub fn build_noise_initiator(
        secret: &'static [u8; 32],
        private_key: &'static [u8],
    ) -> HandshakeState {
        build_noise(secret, private_key).build_initiator().unwrap()
    }
}
