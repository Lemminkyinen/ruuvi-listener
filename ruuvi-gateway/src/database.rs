// ruuvi_measurements=# \d sensor_readings
//                                             Table "public.sensor_readings"
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
// Indexes:
//     "sensor_readings_pkey" PRIMARY KEY, btree (id)

use crate::RuuviV2;
use chrono::{DateTime, Utc};
use sqlx::types::mac_address::MacAddress;
use sqlx::{Pool, Postgres};

pub async fn insert_data(
    pool: &Pool<Postgres>,
    data: RuuviV2,
    timestamp: DateTime<Utc>,
) -> Result<(), anyhow::Error> {
    sqlx::query::<Postgres>(
        r#"
        INSERT INTO sensor_readings (
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
            dew_point_temperature
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(timestamp)
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
    .execute(pool)
    .await?;
    Ok(())
}
