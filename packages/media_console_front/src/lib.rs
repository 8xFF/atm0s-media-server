#[cfg(debug_assertions)]
use poem::EndpointExt;
use poem::Route;

mod dev_proxy;

/// only include in release build
#[cfg(not(debug_assertions))]
#[derive(rust_embed::Embed)]
#[folder = "./react-app/dist"]
pub struct AdminPanelFiles;

pub fn frontend_app() -> Route {
    #[cfg(debug_assertions)]
    {
        let pconfig = dev_proxy::ProxyConfig::new("localhost:5173")
            .web_insecure() // Enables proxy-ing web requests, sets the proxy to use http instead of https
            .enable_nesting() // Sets the proxy to support nested routes
            .finish(); // Finishes constructing the configuration

        // Development mode: spawn Vite dev server
        println!("Running in development mode, starting Vite dev server...");
        std::process::Command::new("pnpm")
            .current_dir(format!("{}/react-app", env!("CARGO_MANIFEST_DIR")))
            .args(&["install"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to install dependencies");

        std::process::Command::new("pnpm")
            .current_dir(format!("{}/react-app", env!("CARGO_MANIFEST_DIR")))
            .args(&["run", "dev"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .expect("Failed to start Vite dev server");

        // Proxy frontend requests to Vite
        Route::new().nest("/", dev_proxy::proxy.data(pconfig)) // You can add your API here
    }

    #[cfg(not(debug_assertions))]
    {
        // Production mode: serve static files
        Route::new()
            .at("/", media_server_utils::EmbeddedFileEndpoint::<AdminPanelFiles>::new("index.html"))
            .nest("/", media_server_utils::EmbeddedFilesEndpoint::<AdminPanelFiles>::new())
    }
}
