// config.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! Configs and associated enums.

use std::fmt::{Debug, Display, Formatter};

use cxx::UniquePtr;

pub use self::ffi::{
    get_device_count, get_log_level, get_random_seed, set_log_level, set_random_seed, BatchType,
    ComputeType, Device, LogLevel,
};

#[cxx::bridge]
pub(crate) mod ffi {
    /// Represents the computing device to be used.
    ///
    /// This enum is a Rust binding to the
    /// [`ctranslate2.Device`](https://opennmt.net/CTranslate2/python/ctranslate2.Device.html),
    /// which can take one of the following two values:
    /// - [`CPU`][Device::CPU]
    /// - [`CUDA`][Device::CUDA]
    ///
    /// The default setting for this enum is [`CPU`][Device::CPU].
    ///
    /// # Examples
    ///
    /// Example of creating a default `Device`:
    ///
    /// ```
    /// use ct2rs::sys::Device;
    ///
    /// let device = Device::default();
    /// # assert_eq!(device, Device::CPU);
    /// ```
    ///
    #[derive(Copy, Clone, Debug)]
    #[repr(i32)]
    enum Device {
        /// Use CPU.
        CPU,
        /// Use GPU (CUDA).
        CUDA,
    }

    /// Model computation type.
    ///
    /// This enum can take one of the following values:
    /// - [`DEFAULT`][ComputeType::DEFAULT]
    /// - [`AUTO`][ComputeType::AUTO]
    /// - [`FLOAT32`][ComputeType::FLOAT32]
    /// - [`INT8`][ComputeType::INT8]
    /// - [`INT8_FLOAT32`][ComputeType::INT8_FLOAT32]
    /// - [`INT8_FLOAT16`][ComputeType::INT8_FLOAT16]
    /// - [`INT8_BFLOAT16`][ComputeType::INT8_BFLOAT16]
    /// - [`INT16`][ComputeType::INT16]
    /// - [`FLOAT16`][ComputeType::FLOAT16]
    /// - [`BFLOAT16`][ComputeType::BFLOAT16]
    ///
    /// The default setting for this enum is [`DEFAULT`][ComputeType::DEFAULT], meaning that unless
    /// specified otherwise, the computation will proceed with the same quantization level as was
    /// used during the model's conversion.
    ///
    /// See also:
    /// [Quantization](https://opennmt.net/CTranslate2/quantization.html#quantize-on-model-loading)
    /// for more details on how quantization affects computation and how it can be applied during
    /// model loading.
    ///
    /// # Examples
    ///
    /// Example of creating a default `ComputeType`:
    ///
    /// ```
    /// use ct2rs::sys::ComputeType;
    ///
    /// let compute_type = ComputeType::default();
    /// # assert_eq!(compute_type, ComputeType::DEFAULT);
    /// ```
    ///
    #[derive(Copy, Clone, Debug)]
    #[repr(i32)]
    enum ComputeType {
        /// Keeps the same quantization that was used during model conversion.
        DEFAULT,
        /// Uses the fastest computation type that is supported on this system and device.
        AUTO,
        /// Utilizes 32-bit floating-point precision.
        FLOAT32,
        /// Uses 8-bit integer precision.
        INT8,
        /// Combines 8-bit integer quantization with 32-bit floating-point computation.
        INT8_FLOAT32,
        /// Combines 8-bit integer quantization with 16-bit floating-point computation.
        INT8_FLOAT16,
        /// Combines 8-bit integer quantization with Brain Floating Point (16-bit) computation.
        INT8_BFLOAT16,
        /// Uses 16-bit integer precision.
        INT16,
        /// Utilizes 16-bit floating-point precision (half precision).
        FLOAT16,
        /// Uses Brain Floating Point (16-bit) precision.
        BFLOAT16,
    }

    /// Specifies how the `max_batch_size` should be calculated.
    ///
    /// This enum can take one of the following two values:
    /// - [`Examples`][BatchType::Examples]
    /// - [`Tokens`][BatchType::Tokens]
    ///
    /// The default setting for this enum is [`Examples`][BatchType::Examples].
    ///
    /// # Examples
    ///
    /// Example of creating a default `BatchType`:
    ///
    /// ```
    /// use ct2rs::sys::BatchType;
    ///
    /// let batch_type = BatchType::default();
    /// # assert_eq!(batch_type, BatchType::Examples);
    /// ```
    #[derive(Copy, Clone, Debug)]
    #[repr(i32)]
    enum BatchType {
        /// The batch size is calculated based on the number of individual examples.
        Examples,
        /// The batch size is calculated based on the total number of tokens across all examples.
        Tokens,
    }

    /// Logging level.
    ///
    /// This enum can take one of the following values:
    /// - [`Off`][LogLevel::Off]
    /// - [`Critical`][LogLevel::Critical]
    /// - [`Error`][LogLevel::Error]
    /// - [`Warning`][LogLevel::Warning]
    /// - [`Info`][LogLevel::Info]
    /// - [`Debug`][LogLevel::Debug]
    /// - [`Trace`][LogLevel::Trace]
    ///
    /// The default setting for this enum is [`Warning`][LogLevel::Warning].
    ///
    /// # Examples
    ///
    /// Example of creating a default `LogLevel`:
    ///
    /// ```
    /// use ct2rs::sys::LogLevel;
    ///
    /// let log_level = LogLevel::default();
    /// # assert_eq!(log_level, LogLevel::Warning);
    /// ```
    #[derive(Copy, Clone, Debug)]
    #[repr(i32)]
    enum LogLevel {
        Off = -3,
        Critical = -2,
        Error = -1,
        Warning = 0,
        Info = 1,
        Debug = 2,
        Trace = 3,
    }

    unsafe extern "C++" {
        include!("ct2rs/include/config.h");

        type Device;
        type ComputeType;
        type ReplicaPoolConfig;
        pub type BatchType;

        fn replica_pool_config(
            num_threads_per_replica: usize,
            max_queued_batches: i32,
            cpu_core_offset: i32,
        ) -> UniquePtr<ReplicaPoolConfig>;

        pub type Config;

        fn config(
            device: Device,
            compute_type: ComputeType,
            device_indices: &[i32],
            tensor_parallel: bool,
            replica_pool_config: UniquePtr<ReplicaPoolConfig>,
        ) -> UniquePtr<Config>;

        /// Returns the number of devices.
        fn get_device_count(device: Device) -> i32;

        type LogLevel;

        /// Sets the CTranslate2 logging level.
        ///
        /// # Examples
        /// The following example sets the log level to `Debug`.
        /// ```
        /// use ct2rs::sys::{LogLevel, set_log_level};
        ///
        /// set_log_level(LogLevel::Debug);
        /// ```
        fn set_log_level(level: LogLevel);

        /// Returns the current logging level.
        fn get_log_level() -> LogLevel;

        /// Sets the seed of random generators.
        ///
        /// # Examples
        /// The following example sets the random seed to `12345`.
        /// ```
        /// use ct2rs::sys::set_random_seed;
        ///
        /// set_random_seed(12345);
        /// ```
        fn set_random_seed(seed: u32);

        /// Returns the current seed of random generators.
        fn get_random_seed() -> u32;
    }
}

impl Default for Device {
    fn default() -> Self {
        Self::CPU
    }
}

impl Display for Device {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match *self {
            Device::CPU => write!(f, "CPU"),
            Device::CUDA => write!(f, "CUDA"),
            _ => write!(f, "Unknown"),
        }
    }
}

impl Default for ComputeType {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Display for ComputeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            ComputeType::DEFAULT => write!(f, "default"),
            ComputeType::AUTO => write!(f, "auto"),
            ComputeType::FLOAT32 => write!(f, "float32"),
            ComputeType::INT8 => write!(f, "int8"),
            ComputeType::INT8_FLOAT32 => write!(f, "int8_float32"),
            ComputeType::INT8_FLOAT16 => write!(f, "int8_float16"),
            ComputeType::INT8_BFLOAT16 => write!(f, "int8_bfloat16"),
            ComputeType::INT16 => write!(f, "int16"),
            ComputeType::FLOAT16 => write!(f, "float16"),
            ComputeType::BFLOAT16 => write!(f, "bfloat16"),
            _ => write!(f, "unknown"),
        }
    }
}

impl Default for BatchType {
    fn default() -> Self {
        Self::Examples
    }
}

impl Display for BatchType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            BatchType::Examples => write!(f, "examples"),
            BatchType::Tokens => write!(f, "tokens"),
            _ => write!(f, "unknown"),
        }
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Warning
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            LogLevel::Off => write!(f, "off"),
            LogLevel::Critical => write!(f, "critical"),
            LogLevel::Error => write!(f, "error"),
            LogLevel::Warning => write!(f, "warning"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Trace => write!(f, "trace"),
            _ => write!(f, "unknown"),
        }
    }
}

/// The `Config` structure holds the configuration settings for CTranslator2.
///
/// # Examples
///
/// Example of creating a default `Config`:
///
/// ```
/// use ct2rs::sys::{ComputeType, Config, Device};
///
/// let config = Config::default();
/// # assert_eq!(config.device, Device::default());
/// # assert_eq!(config.compute_type, ComputeType::default());
/// # assert_eq!(config.device_indices, vec![0]);
/// # assert_eq!(config.tensor_parallel, false);
/// # assert_eq!(config.num_threads_per_replica, 0);
/// # assert_eq!(config.max_queued_batches, 0);
/// # assert_eq!(config.cpu_core_offset, -1);
/// ```
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Config {
    /// Device to use.
    pub device: Device,
    /// Model computation type.
    pub compute_type: ComputeType,
    /// Device IDs where to place this generator on. (default: `vec![0]`)
    pub device_indices: Vec<i32>,
    /// Run model with tensor parallel mode. (default: false)
    pub tensor_parallel: bool,
    /// Number of threads per translator/generator (0 to use a default value). (default: 0)
    pub num_threads_per_replica: usize,
    /// Maximum numbers of batches in the queue (-1 for unlimited, 0 for an automatic value).
    /// When the queue is full, future requests will block until a free slot is available.
    /// (default: 0)
    pub max_queued_batches: i32,
    /// (default: -1)
    pub cpu_core_offset: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device: Default::default(),
            compute_type: Default::default(),
            device_indices: vec![0],
            tensor_parallel: false,
            num_threads_per_replica: 0,
            max_queued_batches: 0,
            cpu_core_offset: -1,
        }
    }
}

impl Config {
    pub(crate) fn to_ffi(&self) -> UniquePtr<ffi::Config> {
        ffi::config(
            self.device,
            self.compute_type,
            self.device_indices.as_slice(),
            false,
            ffi::replica_pool_config(
                self.num_threads_per_replica,
                self.max_queued_batches,
                self.cpu_core_offset,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use rand::random;

    use super::{
        get_device_count, get_log_level, get_random_seed, set_log_level, set_random_seed,
        BatchType, ComputeType, Config, Device, LogLevel,
    };

    #[test]
    fn test_device_display() {
        assert_eq!(format!("{}", Device::CPU), "CPU");
        assert_eq!(format!("{}", Device::CUDA), "CUDA");
    }

    #[test]
    fn test_compute_type_display() {
        assert_eq!(format!("{}", ComputeType::DEFAULT), "default");
        assert_eq!(format!("{}", ComputeType::AUTO), "auto");
        assert_eq!(format!("{}", ComputeType::FLOAT32), "float32");
        assert_eq!(format!("{}", ComputeType::INT8), "int8");
        assert_eq!(format!("{}", ComputeType::INT8_FLOAT32), "int8_float32");
        assert_eq!(format!("{}", ComputeType::INT8_FLOAT16), "int8_float16");
        assert_eq!(format!("{}", ComputeType::INT8_BFLOAT16), "int8_bfloat16");
        assert_eq!(format!("{}", ComputeType::INT16), "int16");
        assert_eq!(format!("{}", ComputeType::FLOAT16), "float16");
        assert_eq!(format!("{}", ComputeType::BFLOAT16), "bfloat16");
    }

    #[test]
    fn test_batch_type_display() {
        assert_eq!(format!("{}", BatchType::Examples), "examples");
        assert_eq!(format!("{}", BatchType::Tokens), "tokens");
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(format!("{}", LogLevel::Off), "off");
        assert_eq!(format!("{}", LogLevel::Critical), "critical");
        assert_eq!(format!("{}", LogLevel::Error), "error");
        assert_eq!(format!("{}", LogLevel::Warning), "warning");
        assert_eq!(format!("{}", LogLevel::Info), "info");
        assert_eq!(format!("{}", LogLevel::Debug), "debug");
        assert_eq!(format!("{}", LogLevel::Trace), "trace");
    }

    #[test]
    fn test_config_to_ffi() {
        let config = Config::default();
        let res = config.to_ffi();

        assert!(!res.is_null());
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_get_device_count() {
        assert_eq!(get_device_count(Device::CPU), 1);
        assert_eq!(get_device_count(Device::CUDA), 0);
    }

    #[test]
    fn test_default_log_level() {
        assert_eq!(LogLevel::default(), LogLevel::Warning);
    }

    #[test]
    fn test_log_level() {
        for l in [
            LogLevel::Off,
            LogLevel::Critical,
            LogLevel::Error,
            LogLevel::Warning,
            LogLevel::Info,
            LogLevel::Debug,
            LogLevel::Trace,
        ] {
            set_log_level(l);
            assert_eq!(get_log_level(), l);
        }
    }

    #[test]
    fn test_random_seed() {
        let r = random::<u32>();
        set_random_seed(r);
        assert_eq!(get_random_seed(), r);
    }
}
