//! GPU metrics collectors

pub mod amd;
pub mod intel;
pub mod nvidia;

pub use amd::AmdGpuCollector;
pub use intel::IntelGpuCollector;
pub use nvidia::NvidiaGpuCollector;
