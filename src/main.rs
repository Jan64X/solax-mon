use serde::{Deserialize};
use std::collections::HashMap;
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct InverterResponse {
    #[serde(rename = "type")]
    inverter_type: i32,
    sn: String,
    ver: String,
    #[serde(rename = "Data")]
    data: Vec<i32>,
    #[serde(rename = "Information")]
    information: Vec<Value>,
}

#[derive(Debug, Clone, Copy)]
enum Units {
    V,
    A,
    W,
    HZ,
    C,
    KWH,
    PERCENT,
    NONE,
}

#[derive(Debug)]
struct Measurement {
    value: f64,
    unit: Units,
}

type TransformFn = fn(f64, Option<&[i32]>) -> f64;

struct X3HybridG4 {
    response_map: HashMap<String, (usize, Units, Option<TransformFn>)>,
}

impl X3HybridG4 {
    fn new() -> Self {
        let mut response_map: HashMap<String, (usize, Units, Option<TransformFn>)> = HashMap::new();
        
        fn div10(x: f64, _: Option<&[i32]>) -> f64 { x / 10.0 }
        fn div100(x: f64, _: Option<&[i32]>) -> f64 { x / 100.0 }
        fn to_signed(x: f64, _: Option<&[i32]>) -> f64 { 
            let x = x as i32;
            f64::from(if x > 32767 { x - 65536 } else { x })
        }
        fn calculate_grid_power(x: f64, data: Option<&[i32]>) -> f64 {
            if let Some(data) = data {
                if let (Some(&high), Some(&low)) = (data.get(34), data.get(35)) {
                    let combined = ((high as i64) << 16) | ((low as i64) & 0xFFFF);
                    if combined > 2147483647 {
                        (combined - 4294967296) as f64
                    } else {
                        combined as f64
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
        
        // Grid measurements
        response_map.insert("Grid 1 Voltage".to_string(), (0, Units::V, Some(div10)));
        response_map.insert("Grid 2 Voltage".to_string(), (1, Units::V, Some(div10)));
        response_map.insert("Grid 3 Voltage".to_string(), (2, Units::V, Some(div10)));
        response_map.insert("Grid 1 Current".to_string(), (3, Units::A, Some(div10)));
        response_map.insert("Grid 2 Current".to_string(), (4, Units::A, Some(div10)));
        response_map.insert("Grid 3 Current".to_string(), (5, Units::A, Some(div10)));
        response_map.insert("Grid 1 Power".to_string(), (6, Units::W, Some(to_signed)));
        response_map.insert("Grid 2 Power".to_string(), (7, Units::W, Some(to_signed)));
        response_map.insert("Grid 3 Power".to_string(), (8, Units::W, Some(to_signed)));
        
        // Solar panel measurements
        response_map.insert("PV1 Voltage".to_string(), (10, Units::V, Some(div10)));
        response_map.insert("PV2 Voltage".to_string(), (11, Units::V, Some(div10)));
        response_map.insert("PV1 Current".to_string(), (12, Units::A, Some(div10)));
        response_map.insert("PV2 Current".to_string(), (13, Units::A, Some(div10)));
        response_map.insert("PV1 Power".to_string(), (14, Units::W, None));
        response_map.insert("PV2 Power".to_string(), (15, Units::W, None));

        // Battery measurements
        response_map.insert("Battery Power".to_string(), (41, Units::W, Some(to_signed)));
        response_map.insert("Battery Remaining Capacity".to_string(), (103, Units::PERCENT, None));
        
        // Home consumption
        response_map.insert("Load/Generator Power".to_string(), (47, Units::W, Some(to_signed)));

        // Grid total power (using indexes 34 and 35)
        response_map.insert("Grid Power".to_string(), (34, Units::W, Some(to_signed)));

        Self { response_map }
    }

    async fn fetch_data(&self, url: &str, password: &str) -> Result<HashMap<String, Measurement>, Box<dyn std::error::Error>> {
        let client = Client::new();
        let params = [("optType", "ReadRealTimeData"), ("pwd", password)];
        
        let response: InverterResponse = client.post(url)
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        let mut measurements = HashMap::new();

        // Process individual measurements
        for (key, (index, unit, transform_fn)) in &self.response_map {
            if let Some(value) = response.data.get(*index) {
                let value = f64::from(*value);
                let final_value = if let Some(transform) = transform_fn {
                    transform(value, Some(&response.data))
                } else {
                    value
                };

                measurements.insert(key.clone(), Measurement {
                    value: final_value,
                    unit: *unit,
                });
            }
        }

        // Calculate combined values
        if let (Some(pv1), Some(pv2)) = (
            measurements.get("PV1 Power"),
            measurements.get("PV2 Power")
        ) {
            measurements.insert("Total Solar Power".to_string(), Measurement {
                value: pv1.value + pv2.value,
                unit: Units::W,
            });
        }

        Ok(measurements)
    }

    fn format_status(&self, measurements: &HashMap<String, Measurement>) -> HashMap<String, String> {
        let mut status = HashMap::new();

        // Total Solar Power
        let solar_power = measurements.get("Total Solar Power")
            .map_or(0.0, |m| m.value);
        status.insert("Solar Panels".to_string(), format!("{:.1}W", solar_power));

        // Battery Status
        let battery_power = measurements.get("Battery Power")
            .map_or(0.0, |m| m.value);
        let battery_status = if battery_power < 0.0 {
            "Discharging"
        } else if battery_power > 0.0 {
            "Charging"
        } else {
            "Idle"
        };
        
        let battery_capacity = measurements.get("Battery Remaining Capacity")
            .map_or(0.0, |m| m.value);
        
        status.insert("Batteries".to_string(), format!("{:.1}%", battery_capacity));
        status.insert("Battery Status".to_string(), battery_status.to_string());
        status.insert("Battery Power".to_string(), format!("{:.1}W", battery_power.abs()));

        // Grid Power Status, swapped import/export because weird api idk
        let grid_power = measurements.get("Grid Power")
            .map_or(0.0, |m| m.value);
        let grid_status = if grid_power < 0.0 {
            "Importing"
        } else if grid_power > 0.0 {
            "Exporting"
        } else {
            "Idle"
        };
        let grid_connection = format!("{:.1}W", grid_power.abs());
        status.insert("Grid".to_string(), format!("{} ({})", grid_status, grid_connection));

        // Home Consumption
        let consumption = measurements.get("Load/Generator Power")
            .map_or(0.0, |m| m.value);
        status.insert("Home Consumption".to_string(), format!("{:.1}W", consumption));

        status
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inverter = X3HybridG4::new();
    let url = "http://10.0.203.11";
    let password = "SERIALHERE";

    match inverter.fetch_data(url, password).await {
        Ok(measurements) => {
            let status = inverter.format_status(&measurements);
            println!("\nFormatted Status:");
            
            // Define the order of status items
            let order = [
                "Solar Panels",
                "Batteries",
                "Battery Status",
                "Battery Power",
                "Grid",
                "Home Consumption"
            ];

            // Print items in the defined order
            for key in order.iter() {
                if let Some(value) = status.get(*key) {
                    println!("{}: {}", key, value);
                }
            }
        },
        Err(e) => eprintln!("Error fetching data: {}", e),
    }

    Ok(())
}