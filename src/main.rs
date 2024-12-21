use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{
    Router,
    routing::get,
    extract::State,
    response::Json,
};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

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

#[derive(Debug, Serialize, Clone)]
struct StatusOutput {
    solar_panels: String,
    batteries: String,
    battery_status: String,
    battery_power: String,
    grid_status: String,
    grid_power: String,
    home_consumption: String,
}

type TransformFn = fn(f64, Option<&[i32]>) -> f64;

struct X3HybridG4 {
    response_map: HashMap<String, (usize, Units, Option<TransformFn>)>,
}

fn read_secrets() -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let mut ip = String::new();
    let mut serial = String::new();
    
    let file = File::open(Path::new("/srv/solax-mon/data/secrets.txt"))?;
    let reader = BufReader::new(file);
    
    for line in reader.lines() {
        let line = line?;
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "INVERTER_IP" => ip = value.trim().to_string(),
                "SERIAL" => serial = value.trim().to_string(),
                _ => (),
            }
        }
    }
    
    if ip.is_empty() || serial.is_empty() {
        return Err("Missing required secrets".into());
    }
    
    Ok((ip, serial))
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
        fn calculate_grid_power(_x: f64, data: Option<&[i32]>) -> f64 {
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

    async fn fetch_data(&self, url: &str, password: &str) -> Result<HashMap<String, Measurement>, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::new();
        let params = [("optType", "ReadRealTimeData"), ("pwd", password)];
        
        let response: InverterResponse = client.post(url)
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        let mut measurements = HashMap::new();

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

    fn format_status(&self, measurements: &HashMap<String, Measurement>) -> StatusOutput {
        let solar_power = measurements.get("Total Solar Power")
            .map_or(0.0, |m| m.value);

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

        let grid_power = measurements.get("Grid Power")
            .map_or(0.0, |m| m.value);
        let grid_status = if grid_power < 0.0 {
            "Importing"
        } else if grid_power > 0.0 {
            "Exporting"
        } else {
            "Idle"
        };

        let consumption = measurements.get("Load/Generator Power")
            .map_or(0.0, |m| m.value);

        StatusOutput {
            solar_panels: format!("{:.1}W", solar_power),
            batteries: format!("{:.1}%", battery_capacity),
            battery_status: battery_status.to_string(),
            battery_power: format!("{:.1}W", battery_power.abs()),
            grid_status: grid_status.to_string(),
            grid_power: format!("{:.1}W", grid_power.abs()),
            home_consumption: format!("{:.1}W", consumption),
        }
    }
}

async fn get_status(
    State(state): State<Arc<RwLock<StatusOutput>>>,
) -> Json<StatusOutput> {
    let status = state.read().await.clone();
    Json(status)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let inverter = X3HybridG4::new();
    
    // Read secrets from file
    let (ip, serial) = read_secrets()?;
    let url = format!("http://{}", ip);

    // Create shared state for the web server
    let shared_status = Arc::new(RwLock::new(StatusOutput {
        solar_panels: "0.0W".to_string(),
        batteries: "0.0%".to_string(),
        battery_status: "Unknown".to_string(),
        battery_power: "0.0W".to_string(),
        grid_status: "Unknown".to_string(),
        grid_power: "0.0W".to_string(),
        home_consumption: "0.0W".to_string(),
    }));

    // Clone the shared state for the background task
    let status_clone = shared_status.clone();

    // Spawn the data collection task
    tokio::spawn(async move {
        loop {
            match inverter.fetch_data(&url, &serial).await {
                Ok(measurements) => {
                    let status = inverter.format_status(&measurements);
                    *status_clone.write().await = status;
                    println!("Data updated successfully");
                },
                Err(e) => eprintln!("Error fetching data: {}", e),
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });

    // Create the router
    let app = Router::new()
        .route("/status", get(get_status))
        .with_state(shared_status);

    // Start the server
    println!("Starting server on http://localhost:3000");
    axum::Server::bind(&"0.0.0.0:3000".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}