use tracing_stackdriver;
use tracing_subscriber::{Registry, layer::SubscriberExt, prelude::*};

pub fn init_logs() {
    let is_prod = std::env::var("K_SERVICE").is_ok();

    let layer = if is_prod {
        tracing_stackdriver::layer().boxed()
    } else {
        tracing_subscriber::fmt::layer().pretty().boxed()
    };

    let subscriber = Registry::default().with(layer);

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");

    tracing::info!("Application starting");
}
