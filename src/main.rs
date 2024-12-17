use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use futures_util::StreamExt;
use reqwest;
use serde_json::Value;
use std::fs::{create_dir_all, File};
use std::io::{self, Write};
use std::path::Path;

#[tokio::main]
async fn main() {
    println!("{}", "===========================================================".green().bold());
    println!("{}", "                    Vizir - Server Manager".yellow().bold());
    println!("{}", "===========================================================".green().bold());
    println!("{}", "Welcome to Vizir! Let's set up your Minecraft server.".blue().bold());
    println!();
    println!("{}", "What type of server would you like to install?".cyan().bold());
    println!("{}", "1. Paper".yellow());
    println!("{}", "2. Purpur".magenta());
    println!();

    let server_type = loop {
        print!("{}", "Enter your choice (1-2): ".cyan());
        io::stdout().flush().unwrap();
        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        match choice.trim() {
            "1" => {
                println!("{}", "✅ You selected Paper!".green());
                break "paper";
            }
            "2" => {
                println!("{}", "✅ You selected Purpur!".purple());
                break "purpur";
            }
            _ => println!("{}", "❌ Invalid choice, please enter 1 or 2.".red()),
        }
    };

    print_separator();
    println!("{}", "Fetching versions...".cyan());
    let versions = fetch_versions(server_type).await;
    if versions.is_empty() {
        println!("{}", "❌ Failed to fetch versions. Please check your internet connection.".red());
        return;
    }

    println!();
    println!("{}", "Available versions:".green());
    for version in &versions {
        println!("{} {}", "•".blue(), version);
    }

    let selected_version = loop {
        print!("{}", "Enter the version number you want to install (e.g., 1.20.1): ".cyan());
        io::stdout().flush().unwrap();
        let mut version_choice = String::new();
        io::stdin().read_line(&mut version_choice).unwrap();
        let version_choice = version_choice.trim();
        if versions.contains(&version_choice.to_string()) {
            println!("{} {}", "✅ You selected version:".green(), version_choice.yellow());
            break version_choice.to_string();
        } else {
            println!("{}", "❌ Invalid version. Please enter a valid version from the list.".red());
        }
    };

    print_separator();
    println!("{}", "Fetching builds...".cyan());
    let builds = fetch_builds(server_type, &selected_version).await;
    if builds.is_empty() {
        println!("{} {}.", "❌ No builds found for the selected version".red(), selected_version);
        println!("{}", "Please restart the program and try selecting another version.".red());
        return;
    }
    println!("{}", "Available builds:".green());
    for build in &builds {
        println!("{} Build {}", "•".blue(), build);
    }
    let latest_build = *builds.last().unwrap();
    println!("{} {} {} {}.", "✅ The latest available build for version".green(), selected_version.yellow(), "is".green(), latest_build.to_string().yellow());
    print_separator();
    println!("{}", "Downloading the latest build...".cyan());
    println!("{}", "Where would you like to save the server files?".cyan());
    print!("{}", "Enter the folder path: ".cyan());
    io::stdout().flush().unwrap();
    let mut folder_path = String::new();
    io::stdin().read_line(&mut folder_path).unwrap();
    let folder_path = folder_path.trim();
    if !Path::new(folder_path).exists() {
        create_dir_all(folder_path).expect("Failed to create the directory");
    }
    let success = download_server_jar(server_type, &selected_version, latest_build, folder_path).await;
    if !success {
        println!("{}", "❌ Failed to download the server jar!".red());
        return;
    }
    println!("{}", "✅ Successfully downloaded the server jar!".green());
    print_separator();
    println!("{}", "How much RAM should be allocated to the server? (e.g., 4G, 8G)".cyan());
    print!("{}", "Enter RAM: ".cyan());
    io::stdout().flush().unwrap();
    let mut ram = String::new();
    io::stdin().read_line(&mut ram).unwrap();
    let ram = ram.trim();
    println!("{}", "Do you want a GUI? (yes/no)".cyan());
    print!("{}", "Enter your choice: ".cyan());
    io::stdout().flush().unwrap();
    let mut gui_choice = String::new();
    io::stdin().read_line(&mut gui_choice).unwrap();
    let gui_choice = gui_choice.trim().to_lowercase();
    let use_gui = gui_choice == "yes";

    let bat_file_content = format!(
        "@echo off\n\
         title Vizir - Minecraft Server\n\
         java -Xmx{ram} -jar server.jar {}\n\
         pause",
        if use_gui { "" } else { "nogui" }
    );
    let bat_file_path = format!("{}/start_server.bat", folder_path);
    let mut bat_file = File::create(&bat_file_path).expect("Failed to create .bat file");
    bat_file.write_all(bat_file_content.as_bytes()).expect("Failed to write to .bat file");
    println!();
    println!("{}", "🎉 Server setup complete!".green().bold());
    println!("{} {}", "➡️ To start the server, run the file:".cyan(), bat_file_path.yellow());
    print_separator();
    println!("{}", "Thank you for using Vizir Server Manager!".blue().bold());
    println!("{}", "Press Enter to exit the program...".cyan());
    let mut exit_input = String::new();
    io::stdin().read_line(&mut exit_input).unwrap();
}

fn print_separator() {
    println!("{}", "===========================================================".blue().bold());
}

async fn fetch_versions(project: &str) -> Vec<String> {
    let api_url = match project {
        "paper" => "https://api.papermc.io/v2/projects/paper",
        "purpur" => "https://api.purpurmc.org/v2/purpur",
        _ => {
            eprintln!("Unknown project type: '{}'", project);
            return vec![];
        }
    };
    let response = reqwest::get(api_url).await;
    let json: Value = match response {
        Ok(resp) if resp.status().is_success() => match resp.json().await {
            Ok(data) => data,
            Err(e) => {
                eprintln!("❌ Failed to parse JSON response: {}", e);
                return vec![];
            }
        },
        Ok(resp) => {
            eprintln!("❌ Request failed: HTTP {}", resp.status());
            return vec![];
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to API: {}", e);
            return vec![];
        }
    };
    json["versions"].as_array().map_or(vec![], |versions| {
        versions.iter().filter_map(|v| v.as_str().map(String::from)).collect()
    })
}

async fn fetch_builds(project: &str, version: &str) -> Vec<u64> {
    let api_url = match project {
        "purpur" => format!("https://api.purpurmc.org/v2/purpur/{}", version),
        "paper" => format!("https://api.papermc.io/v2/projects/{}/versions/{}/builds", project, version),
        _ => {
            eprintln!("Unknown project type: '{}'", project);
            return vec![];
        }
    };
    let response = match reqwest::get(&api_url).await {
        Ok(resp) => resp,
        Err(e) => {
            println!("❌ Failed to fetch builds: {}", e);
            return vec![];
        }
    };
    let json: Value = match response.json().await {
        Ok(data) => data,
        Err(e) => {
            println!("❌ Failed to parse JSON response: {}", e);
            return vec![];
        }
    };
    if project == "purpur" {
        json["builds"]["all"].as_array().map_or(vec![], |builds| {
            builds.iter().filter_map(|b| b.as_str().and_then(|s| s.parse::<u64>().ok())).collect()
        })
    } else if project == "paper" {
        json["builds"].as_array().map_or(vec![], |builds| {
            builds.iter().filter_map(|b| b["build"].as_u64()).collect()
        })
    } else {
        vec![]
    }
}

async fn download_server_jar(project: &str, version: &str, build: u64, output_dir: &str) -> bool {
    let api_url = match project {
        "purpur" => format!("https://api.purpurmc.org/v2/purpur/{}/{}/download", version, build),
        "paper" => format!("https://api.papermc.io/v2/projects/{}/versions/{}/builds/{}/downloads/paper-{}-{}.jar", project, version, build, version, build),
        _ => {
            println!("❌ Unknown project type: {}", project);
            return false;
        }
    };
    let output_path = format!("{}/server.jar", output_dir);
    let client = reqwest::Client::new();
    let response = match client.get(&api_url).send().await {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) => {
            println!("❌ Request failed: HTTP {}", resp.status());
            return false;
        }
        Err(e) => {
            println!("❌ Failed to connect to API: {}", e);
            return false;
        }
    };
    let total_size = match response.content_length() {
        Some(size) => size,
        None => {
            println!("❌ Failed to get the file size.");
            return false;
        }
    };
    let mut file = match File::create(&output_path) {
        Ok(file) => file,
        Err(e) => {
            println!("❌ Failed to create file: {}", e);
            return false;
        }
    };
    let mut stream = response.bytes_stream();
    let progress_bar = ProgressBar::new(total_size);
    progress_bar.set_style(ProgressStyle::default_bar().template("{spinner:.cyan} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})").unwrap().progress_chars("#>-"));
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if let Err(e) = file.write_all(&bytes) {
                    println!("❌ Failed to write to file: {}", e);
                    return false;
                }
                progress_bar.inc(bytes.len() as u64);
            }
            Err(e) => {
                println!("❌ Error during file download: {}", e);
                return false;
            }
        }
    }
    progress_bar.finish_with_message("Download completed!");
    true
}