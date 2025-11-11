use crate::{RuuviE1, RuuviV2};
use sqlx::types::mac_address::MacAddress;
use sqlx::{Pool, Postgres};

// ruuvi_measurements=# \d tag_readings
//                                               Table "public.tag_readings"
//         Column         |           Type           | Collation | Nullable |                   Default
// -----------------------+--------------------------+-----------+----------+---------------------------------------------
//  id                    | integer                  |           | not null | nextval('sensor_readings_id_seq'::regclass)
//  recorded_at           | timestamp with time zone |           | not null | now()
//  mac_address           | macaddr                  |           | not null |
//  temperature           | real                     |           |          |
//  relative_humidity     | real                     |           |          |
//  pressure              | integer                  |           |          |
//  acceleration_x        | smallint                 |           |          |
//  acceleration_y        | smallint                 |           |          |
//  acceleration_z        | smallint                 |           |          |
//  battery_voltage       | real                     |           |          |
//  tx_power              | smallint                 |           |          |
//  movement_counter      | smallint                 |           |          |
//  measurement_sequence  | integer                  |           |          |
//  absolute_humidity     | real                     |           |          |
//  dew_point_temperature | real                     |           |          |
//  rssi                  | smallint                 |           |          |

pub async fn insert_data_v2(pool: &Pool<Postgres>, data: RuuviV2) -> Result<(), anyhow::Error> {
    sqlx::query::<Postgres>(
        r#"
        INSERT INTO tag_readings (
            recorded_at,
            mac_address,
            temperature,
            relative_humidity,
            pressure,
            acceleration_x,
            acceleration_y,
            acceleration_z,
            battery_voltage,
            tx_power,
            movement_counter,
            measurement_sequence,
            absolute_humidity,
            dew_point_temperature,
            rssi
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#,
    )
    .bind(data.timestamp)
    .bind(MacAddress::new(data.mac))
    .bind(data.temp)
    .bind(data.rel_humidity)
    .bind(data.abs_pressure as i32)
    .bind(data.acc_x)
    .bind(data.acc_y)
    .bind(data.acc_z)
    .bind(data.battery_voltage)
    .bind(data.tx_power as i16)
    .bind(data.movement_counter as i16)
    .bind(data.measurement_seq as i32)
    .bind(data.abs_humidity as f32)
    .bind(data.dew_point_temp as f32)
    .bind(data.rssi as i16)
    .execute(pool)
    .await?;
    Ok(())
}

// ruuvi_measurements=# \d air_readings
//                                             Table "public.air_readings"
//         Column         |           Type           | Collation | Nullable |                 Default
// -----------------------+--------------------------+-----------+----------+------------------------------------------
//  id                    | integer                  |           | not null | nextval('air_readings_id_seq'::regclass)
//  recorded_at           | timestamp with time zone |           | not null | now()
//  mac_address           | macaddr                  |           | not null |
//  temperature           | real                     |           |          |
//  dew_point_temperature | double precision         |           |          |
//  relative_humidity     | real                     |           |          |
//  absolute_humidity     | double precision         |           |          |
//  pressure              | integer                  |           |          |
//  pm1_0                 | real                     |           |          |
//  pm2_5                 | real                     |           |          |
//  pm4_0                 | real                     |           |          |
//  pm10_0                | real                     |           |          |
//  co2                   | smallint                 |           |          |
//  voc_index             | smallint                 |           |          |
//  nox_index             | smallint                 |           |          |
//  luminosity            | real                     |           |          |
//  measurement_sequence  | integer                  |           |          |
//  flags                 | smallint                 |           |          |
//  tx_power              | smallint                 |           |          |
//  rssi                  | smallint                 |           |          |

pub async fn insert_data_e1(pool: &Pool<Postgres>, data: RuuviE1) -> Result<(), anyhow::Error> {
    sqlx::query::<Postgres>(
        r#"
        INSERT INTO air_readings (
            recorded_at,
            mac_address,
            temperature,
            dew_point_temperature,
            relative_humidity,
            absolute_humidity,
            pressure,
            pm1_0,
            pm2_5,
            pm4_0,
            pm10_0,
            co2,
            voc_index,
            nox_index,
            luminosity,
            measurement_sequence,
            flags,
            tx_power,
            rssi
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19
        )
        "#,
    )
    .bind(data.timestamp)
    .bind(MacAddress::new(data.mac))
    .bind(data.temp)
    .bind(data.dew_point_temp)
    .bind(data.rel_humidity)
    .bind(data.abs_humidity)
    .bind(data.abs_pressure as i32)
    .bind(data.pm1_0)
    .bind(data.pm2_5)
    .bind(data.pm4_0)
    .bind(data.pm10_0)
    .bind(data.co2 as i16)
    .bind(data.voc_index as i16)
    .bind(data.nox_index as i16)
    .bind(data.luminosity)
    .bind(data.measurement_seq as i32)
    .bind(data.flags as i16)
    .bind(data.tx_power as i16)
    .bind(data.rssi as i16)
    .execute(pool)
    .await?;
    Ok(())
}
