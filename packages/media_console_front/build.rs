use std::process::Command;

fn main() {
    // Build Vite project when compiling in release mode
    if !cfg!(debug_assertions) {
        Command::new("pnpm")
            .current_dir(format!("{}/react-app", env!("CARGO_MANIFEST_DIR")))
            .args(&["install"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .expect("Failed to install Vite project");
        Command::new("pnpm")
            .current_dir(format!("{}/react-app", env!("CARGO_MANIFEST_DIR")))
            .args(&["run", "build"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .expect("Failed to build Vite project");
    }
}
