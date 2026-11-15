//! CLI command: Display hardware capabilities, proven encoders, and decoders.

use ventacore_platform::capabilities::probe_hardware_capabilities;

pub fn execute_capabilities() {
    let report = probe_hardware_capabilities();

    println!("============================================================");
    println!(" VentaCore Hardware & Graphics Capabilities");
    println!("============================================================");
    println!("Graphics Adapters ({}):", report.adapters.len());
    for (idx, adapter) in report.adapters.iter().enumerate() {
        println!(
            "  [{}] {} (Backend: {}, Type: {}, Driver: {})",
            idx,
            adapter.name,
            adapter.backend,
            adapter.device_type,
            adapter.driver_info
        );
    }

    println!("\nSupported Video Decoders (Proven by Trial):");
    for dec in &report.decoders {
        println!(
            "  - {} via {}: {:?}",
            dec.codec, dec.hw_accel, dec.status
        );
    }

    println!("\nSupported Video Encoders (Proven by Trial):");
    for enc in &report.encoders {
        println!(
            "  - {} via {}: {:?}",
            enc.codec, enc.hw_accel, enc.status
        );
    }
    println!("============================================================");
}
