//! Stage 19: Hardware Proof Integration Test
//!
//! Verifies that `measure-hardware` runs, probes adapters and codecs,
//! benchmarks the CPU reference oracle, and verifies 0-tick rounding errors.

use ventacore_cli::commands::measure_hardware::execute_measure_hardware;

#[test]
fn test_measure_hardware_subcommand_execution() {
    let result = execute_measure_hardware();
    assert!(result.is_ok(), "measure-hardware must succeed on physical target");
}
