use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Only install control script when installed via cargo install
    if env::var("CARGO_INSTALL_ROOT").is_ok() {
        install_control_script();
    }
    
    // Re-run build script if control script changes
    println!("cargo:rerun-if-changed=scripts/bevy-debugger-control");
}

fn install_control_script() {
    // Get the cargo install bin directory
    let install_root = match env::var("CARGO_INSTALL_ROOT") {
        Ok(root) => PathBuf::from(root),
        Err(_) => {
            // Fallback to default cargo bin location
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".cargo")
        }
    };
    
    let bin_dir = install_root.join("bin");
    let script_dest = bin_dir.join("bevy-debugger-control");
    
    // Read the control script from the package
    let script_src = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("scripts")
        .join("bevy-debugger-control");
    
    // Only install if source exists
    if script_src.exists() {
        match fs::read_to_string(&script_src) {
            Ok(content) => {
                // Write the script to the bin directory
                if let Err(e) = fs::write(&script_dest, content) {
                    eprintln!("Warning: Failed to install control script: {}", e);
                    return;
                }
                
                // Make the script executable on Unix-like systems
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = fs::metadata(&script_dest) {
                        let mut permissions = metadata.permissions();
                        permissions.set_mode(0o755);
                        let _ = fs::set_permissions(&script_dest, permissions);
                    }
                }
                
                println!("cargo:warning=Installed bevy-debugger-control script to {:?}", script_dest);
            }
            Err(e) => {
                eprintln!("Warning: Failed to read control script: {}", e);
            }
        }
    }
}