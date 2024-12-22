use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;
use reqwest;
use anyhow::{Result, Context};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug)]
struct PowerStatus {
    solar_panels: String,
    batteries: String,
    battery_status: String,
    battery_power: String,
    grid_status: String,
    grid_power: String,
    home_consumption: String,
}

#[derive(Debug)]
struct IdracConfig {
    enabled: bool,
    servers: Vec<IdracServer>,
}

#[derive(Debug)]
struct IdracServer {
    ip: String,
    username: String,
    password: String,
}

#[derive(Debug)]
struct Config {
    servers: Vec<String>,
    ssh_key_path: String,
    discord_webhook_url: String,
    idrac: IdracConfig,
}

async fn send_discord_alert(webhook_url: &str, message: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let payload = json!({
        "content": message
    });

    let response = client.post(webhook_url)
        .json(&payload)
        .send()
        .await
        .context("Failed to send Discord webhook request")?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        anyhow::bail!(
            "Discord webhook failed with status {}: {}", 
            status,
            error_text
        );
    }

    Ok(())
}

async fn shutdown_server(server: &str, ssh_key_path: &str) -> Result<()> {
    let output = Command::new("ssh")
        .args([
            "-i", ssh_key_path,
            "-o", "StrictHostKeyChecking=no",
            server,
            "sudo poweroff"
        ])
        .output()
        .context("Failed to execute SSH command")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to shutdown server {}: {}", server, error);
    }

    Ok(())
}

async fn power_on_idrac(server: &IdracServer) -> Result<()> {
    let output = Command::new("sshpass")
        .args([
            "-p", &server.password,
            "ssh",
            "-o", "StrictHostKeyChecking=no",
            &format!("{}@{}", server.username, server.ip),
            "racadm serveraction powerup"
        ])
        .output()
        .context("Failed to execute iDRAC power-on command")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to power on iDRAC server {}: {}", server.ip, error);
    }

    Ok(())
}

fn parse_power_value(value: &str) -> f64 {
    value.trim_end_matches('W')
        .parse::<f64>()
        .unwrap_or(0.0)
}

fn parse_battery_percentage(value: &str) -> f64 {
    value.trim_end_matches('%')
        .parse::<f64>()
        .unwrap_or(0.0)
}

fn load_config() -> Result<Config> {
    let config_content = fs::read_to_string("/srv/solax-mon/data/secrets.txt")
        .context("Failed to read config file")?;
    
    let mut servers = Vec::new();
    let mut discord_webhook_url = String::new();
    let mut have_idrac = false;
    let mut idrac_servers = Vec::new();
    
    for line in config_content.lines() {
        let line = line.trim();
        if line.starts_with("SERVER=") {
            servers.push(line.trim_start_matches("SERVER=").to_string());
        } else if line.starts_with("DISCORD_WEBHOOK=") {
            discord_webhook_url = line.trim_start_matches("DISCORD_WEBHOOK=").to_string();
        } else if line.starts_with("HAVE_IDRAC=") {
            have_idrac = line.trim_start_matches("HAVE_IDRAC=").to_lowercase() == "true";
        } else if line.starts_with("IDRAC_SERVER=") {
            let parts: Vec<&str> = line.trim_start_matches("IDRAC_SERVER=").split(',').collect();
            if parts.len() == 3 {
                idrac_servers.push(IdracServer {
                    ip: parts[0].to_string(),
                    username: parts[1].to_string(),
                    password: parts[2].to_string(),
                });
            }
        }
    }

    Ok(Config {
        servers,
        ssh_key_path: "/srv/solax-mon/data/ssh.key".to_string(),
        discord_webhook_url,
        idrac: IdracConfig {
            enabled: have_idrac,
            servers: idrac_servers,
        },
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting power monitoring service...");
    let config = load_config()?;
    println!("Loaded configuration with {} servers", config.servers.len());
    if config.idrac.enabled {
        println!("iDRAC support enabled with {} servers", config.idrac.servers.len());
    }
    
    let client = reqwest::Client::new();
    let mut shutdown_triggered = false;
    let mut iteration = 1;

    loop {
        println!("\n=== Monitoring Iteration {} ===", iteration);
        
        match client.get("http://localhost:3000/status")
            .send()
            .await {
                Ok(response) => {
                    if let Ok(status) = response.json::<PowerStatus>().await {
                        // Print current status
                        println!("Current Power Status:");
                        println!("├─ Solar Output: {}", status.solar_panels);
                        println!("├─ Battery Level: {}", status.batteries);
                        println!("├─ Battery Status: {}", status.battery_status);
                        println!("├─ Battery Power: {}", status.battery_power);
                        println!("├─ Grid Status: {}", status.grid_status);
                        println!("├─ Grid Power: {}", status.grid_power);
                        println!("└─ Home Consumption: {}", status.home_consumption);

                        let grid_power = parse_power_value(&status.grid_power);
                        let solar_power = parse_power_value(&status.solar_panels);
                        let home_power = parse_power_value(&status.home_consumption);
                        let battery_percentage = parse_battery_percentage(&status.batteries);

                        // Print threshold status
                        println!("\nThreshold Check:");
                        println!("├─ Grid Power == 0W: {}", grid_power == 0.0);
                        println!("├─ Solar Power < Home Consumption ({} < {}): {}", 
                            solar_power, home_power, solar_power < home_power);
                        println!("└─ Battery < 10%: {}", battery_percentage < 10.0);

                        let critical_condition = grid_power == 0.0 && 
                                              solar_power < home_power && 
                                              battery_percentage < 10.0;

                        if critical_condition {
                            println!("\n🚨 CRITICAL: All shutdown conditions met!");
                            if !shutdown_triggered {
                                println!("Initiating shutdown sequence...");
                                
                                // Send Discord alert
                                let alert_message = format!(
                                    "🚨 CRITICAL POWER ALERT!\n\
                                    Grid: {}W (Offline)\n\
                                    Solar: {}W\n\
                                    Home Consumption: {}W\n\
                                    Battery: {}%\n\
                                    \n\
                                    ⚠️ Initiating server shutdown sequence...",
                                    grid_power, solar_power, home_power, battery_percentage
                                );
                                
                                match send_discord_alert(&config.discord_webhook_url, &alert_message).await {
                                    Ok(_) => println!("Successfully sent Discord alert"),
                                    Err(e) => {
                                        eprintln!("Failed to send Discord alert:");
                                        eprintln!("Error details: {}", e);
                                        let masked_url = if config.discord_webhook_url.len() > 20 {
                                            format!("{}...{}", 
                                                &config.discord_webhook_url[..10],
                                                &config.discord_webhook_url[config.discord_webhook_url.len()-10..])
                                        } else {
                                            "Invalid URL".to_string()
                                        };
                                        eprintln!("Webhook URL (masked): {}", masked_url);
                                    }
                                }

                                // Shutdown servers
                                for server in &config.servers {
                                    match shutdown_server(server, &config.ssh_key_path).await {
                                        Ok(_) => println!("Successfully initiated shutdown for {}", server),
                                        Err(e) => eprintln!("Failed to shutdown {}: {}", server, e),
                                    }
                                }
                                
                                shutdown_triggered = true;
                            } else {
                                println!("Shutdown already triggered, waiting for conditions to normalize...");
                            }
                        } else {
                            if shutdown_triggered {
                                println!("\nConditions normalized, initiating recovery sequence");
                                
                                // Send normalization alert
                                let normal_message = format!(
                                    "✅ Power conditions normalized!\n\
                                    Grid: {}W\n\
                                    Solar: {}W\n\
                                    Home Consumption: {}W\n\
                                    Battery: {}%\n",
                                    grid_power, solar_power, home_power, battery_percentage
                                );

                                match send_discord_alert(&config.discord_webhook_url, &normal_message).await {
                                    Ok(_) => println!("Successfully sent normalization alert"),
                                    Err(e) => eprintln!("Failed to send normalization alert: {}", e),
                                }

                                // Power on iDRAC servers if enabled
                                if config.idrac.enabled {
                                    println!("Initiating iDRAC power-on sequence...");
                                    for server in &config.idrac.servers {
                                        match power_on_idrac(server).await {
                                            Ok(_) => println!("Successfully powered on iDRAC server {}", server.ip),
                                            Err(e) => eprintln!("Failed to power on iDRAC server {}: {}", server.ip, e),
                                        }
                                    }
                                }

                                shutdown_triggered = false;
                            } else {
                                println!("\nOperating within normal parameters");
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Failed to fetch power status: {}", e),
            }

        iteration += 1;
        println!("\nWaiting 30 seconds before next check...");
        thread::sleep(Duration::from_secs(30));
    }
}