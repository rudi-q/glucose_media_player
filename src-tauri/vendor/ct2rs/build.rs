// build.rs
//
// Copyright (c) 2023-2025 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

use std::env;
use std::path::{Path, PathBuf};

use cmake::Config;
use walkdir::WalkDir;

fn add_search_paths(key: &str) {
    println!("cargo:rerun-if-env-changed={}", key);
    if let Ok(library_path) = env::var(key) {
        env::split_paths(&library_path)
            .filter(|v| v.exists())
            .for_each(|v| {
                println!("cargo:rustc-link-search={}", v.display());
            });
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum Os {
    Win,
    Mac,
    Linux,
    Unknown,
}

fn build_ctranslate2() {
    let mut cmake = Config::new("CTranslate2");
    match env::var("CMAKE_PARALLEL") {
        Ok(job_n) => {
            cmake.build_arg("-j").build_arg(job_n);
        }
        Err(env::VarError::NotPresent) => (),
        Err(err) => panic!("CMAKE_PARALLEL format error: {:?}", err),
    }
    let os = if cfg!(target_os = "windows") {
        Os::Win
    } else if cfg!(target_os = "macos") {
        Os::Mac
    } else if cfg!(target_os = "linux") {
        Os::Linux
    } else {
        Os::Unknown
    };

    let aarch64 = cfg!(target_arch = "aarch64");

    let cuda = cfg!(feature = "cuda");
    let cudnn = cfg!(feature = "cudnn");
    let cuda_dynamic_loading = cfg!(feature = "cuda-dynamic-loading");
    let mkl = cfg!(feature = "mkl");
    let openblas = cfg!(feature = "openblas");
    let ruy = cfg!(feature = "ruy");
    let accelarate = cfg!(feature = "accelerate");
    let tensor_parallel = cfg!(feature = "tensor-parallel");
    let dnnl = cfg!(feature = "dnnl");
    let mut openmp_comp: bool = cfg!(feature = "openmp-runtime-comp");
    let openmp_intel = cfg!(feature = "openmp-runtime-intel");
    let sse4_1 = cfg!(target_feature = "sse4.1");
    if !openmp_intel && !openmp_comp && dnnl {
        if os == Os::Linux {
            openmp_comp = true;
        }
    }
    let flash_attention = cfg!(feature = "flash-attention");
    cmake
        .define("BUILD_CLI", "OFF")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("WITH_MKL", "OFF")
        .define("OPENMP_RUNTIME", "NONE")
        .define("CMAKE_POLICY_VERSION_MINIMUM", "3.5");
    if os == Os::Win {
        // Glucose: build against the dynamic C runtime like whisper.cpp and the rest of
        // the app, so both libraries link into one binary without /FORCE:MULTIPLE.
        let static_crt = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default().contains("crt-static");
        cmake.profile("Release").cxxflag("/EHsc").static_crt(static_crt);
        cmake.define(
            "CMAKE_MSVC_RUNTIME_LIBRARY",
            if static_crt { "MultiThreaded" } else { "MultiThreadedDLL" },
        );
    }
    let mut include_paths: Vec<PathBuf> = env::var("CMAKE_INCLUDE_PATH")
        .as_ref()
        .map(|v| env::split_paths(v).collect())
        .unwrap_or_default();
    let mut library_paths: Vec<PathBuf> = env::var("CMAKE_LIBRARY_PATH")
        .as_ref()
        .map(|v| env::split_paths(v).collect())
        .unwrap_or_default();

    if cuda {
        let cuda = cuda_root().expect("CUDA_TOOLKIT_ROOT_DIR is not specified");
        cmake.define("WITH_CUDA", "ON");
        cmake.define("CUDA_TOOLKIT_ROOT_DIR", &cuda);
        let arch_config = resolve_cuda_arch_list();
        cmake.define("CUDA_ARCH_LIST", &arch_config.cmake_value);
        let mut nvcc_flags = Vec::new();
        if cfg!(feature = "cuda-small-binary") {
            nvcc_flags.push("-Xfatbin=-compress-all".to_string());
        }
        nvcc_flags.extend(arch_config.extra_nvcc_flags);
        if !nvcc_flags.is_empty() {
            cmake.define("CUDA_NVCC_FLAGS", nvcc_flags.join(";"));
        }
        println!("cargo:rustc-link-search={}", cuda.join("lib").display());
        println!("cargo:rustc-link-search={}", cuda.join("lib64").display());
        println!("cargo:rustc-link-search={}", cuda.join("lib/x64").display());
        println!("cargo:rustc-link-lib=static=cudart_static");
        if cudnn {
            cmake.define("WITH_CUDNN", "ON");
            println!("cargo:rustc-link-lib=cudnn");
        }
        if cuda_dynamic_loading {
            cmake.define("CUDA_DYNAMIC_LOADING", "ON");
        } else {
            if os == Os::Win {
                println!("cargo:rustc-link-lib=static=cublas");
                println!("cargo:rustc-link-lib=static=cublasLt");
            } else {
                println!("cargo:rustc-link-lib=static=cublas_static");
                println!("cargo:rustc-link-lib=static=cublasLt_static");
                println!("cargo:rustc-link-lib=static=culibos");
            }
        }
    }
    if os == Os::Mac && aarch64 {
        cmake.define("CMAKE_OSX_ARCHITECTURES", "arm64");
    }

    if mkl {
        cmake.define("WITH_MKL", "ON");
        if let Ok(mklroot) = env::var("DEP_MKL_ROOT") {
            cmake.env("MKLROOT", mklroot);
        }
        if let Ok(include_path) = env::var("DEP_MKL_INCLUDE_PATH") {
            include_paths.push(PathBuf::from(include_path));
        }
        if let Ok(library_path) = env::var("DEP_MKL_LIBRARY_PATH") {
            library_paths.push(PathBuf::from(library_path));
        }
    }
    if openblas {
        cmake.define("WITH_OPENBLAS", "ON");
        if os != Os::Win {
            include_paths.push(PathBuf::from(env::var("DEP_OPENBLAS_INCLUDE").unwrap()));
            library_paths.push(PathBuf::from(env::var("DEP_OPENBLAS_LIBRARY").unwrap()));
        }
    }
    if ruy {
        cmake.define("WITH_RUY", "ON");
    }
    if accelarate {
        println!("cargo:rustc-link-lib=framework=Accelerate");
        cmake.define("WITH_ACCELERATE", "ON");
    }
    if tensor_parallel {
        cmake.define("WITH_TENSOR_PARALLEL", "ON");
    }
    if sse4_1 {
        cmake.define("CMAKE_CXX_FLAGS", "-msse4.1");
    }
    if dnnl {
        cmake.define("WITH_DNNL", "ON");
        include_paths.push(PathBuf::from(env::var("DEP_DNNL_INCLUDE_PATH").unwrap()));
        library_paths.push(PathBuf::from(env::var("DEP_DNNL_LIBRARY_PATH").unwrap()));
    }
    if openmp_comp {
        // `gomp` is GNU's OpenMP runtime and does not exist on the MSVC toolchain,
        // where linking it fails with `LNK1181: cannot open input file 'gomp.lib'`.
        // MSVC's `/openmp` emits a `/DEFAULTLIB:VCOMP` directive into every object, so
        // its OpenMP runtime is linked automatically and nothing is needed here. Other
        // toolchains, including `*-pc-windows-gnu` (MinGW/GCC), still require it.
        if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
            println!("cargo:rustc-link-lib=gomp");
        }
        cmake.define("OPENMP_RUNTIME", "COMP");
    } else if openmp_intel {
        println!("cargo:rustc-link-lib=iomp5");
        cmake.define("OPENMP_RUNTIME", "INTEL");
    }
    if flash_attention {
        cmake.define("WITH_FLASH_ATTN", "ON");
    }
    if cfg!(feature = "disable-cpu-dispatch") {
        cmake.define("ENABLE_CPU_DISPATCH", "OFF");
    }

    if !include_paths.is_empty() {
        cmake.env(
            "CMAKE_INCLUDE_PATH",
            env::join_paths(include_paths).unwrap(),
        );
    }
    if !library_paths.is_empty() {
        cmake.env(
            "CMAKE_LIBRARY_PATH",
            env::join_paths(library_paths).unwrap(),
        );
    }

    let ctranslate2 = cmake.build();
    link_libraries(ctranslate2.join("build"));
}

struct CudaArchConfig {
    cmake_value: String,
    extra_nvcc_flags: Vec<String>,
}

fn resolve_cuda_arch_list() -> CudaArchConfig {
    let raw = env::var("CUDA_ARCH_LIST")
        .or_else(|_| env::var("CT2_CUDA_ARCH_LIST"))
        .unwrap_or_else(|_| "Common".to_string());
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return CudaArchConfig {
            cmake_value: "Common".to_string(),
            extra_nvcc_flags: Vec::new(),
        };
    }

    let mut cmake_tokens = Vec::new();
    let mut extra_flags = Vec::new();

    for token in trimmed.split(|c: char| c == ';' || c == ',' || c.is_whitespace()) {
        if token.is_empty() {
            continue;
        }
        if let Some(arch) = parse_cuda_arch(token) {
            if arch.major < 10 {
                cmake_tokens.push(format!("{}.{}", arch.major, arch.minor));
            } else {
                extra_flags.push(format!(
                    "-gencode=arch=compute_{major}{minor},code=sm_{major}{minor}",
                    major = arch.major,
                    minor = arch.minor
                ));
            }
        } else {
            cmake_tokens.push(token.trim().to_string());
        }
    }

    if cmake_tokens.is_empty() {
        cmake_tokens.push("Common".to_string());
    }

    CudaArchConfig {
        cmake_value: cmake_tokens.join(";"),
        extra_nvcc_flags: extra_flags,
    }
}

#[derive(Copy, Clone)]
struct ParsedArch {
    major: u32,
    minor: u32,
}

fn parse_cuda_arch(token: &str) -> Option<ParsedArch> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("compute_") {
        return parse_arch_pair(rest.parse::<u32>().ok()?);
    }
    if trimmed.contains('.') {
        let mut parts = trimmed.split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor_part = parts.next().unwrap_or("0");
        let minor_char = minor_part
            .chars()
            .find(|c| c.is_ascii_digit())
            .unwrap_or('0');
        let minor = minor_char.to_digit(10)? as u32;
        return Some(ParsedArch { major, minor });
    }
    if let Ok(value) = trimmed.parse::<u32>() {
        return parse_arch_pair(value);
    }
    None
}

fn parse_arch_pair(value: u32) -> Option<ParsedArch> {
    if value < 10 {
        return None;
    }
    Some(ParsedArch {
        major: value / 10,
        minor: value % 10,
    })
}

fn link_system_libraries() {
    println!("cargo:rustc-link-lib=ctranslate2");
    if cfg!(target_arch = "x86_64") {
        println!("cargo:rustc-link-lib=cpu_features");
    }
    if cfg!(feature = "cuda") {
        println!("cargo:rustc-link-lib=static=cudart_static");
    }
    if cfg!(feature = "cudnn") {
        println!("cargo:rustc-link-lib=cudnn");
    }
    if cfg!(all(
        not(feature = "cuda-dynamic-loading"),
        not(target_os = "macos")
    )) {
        if cfg!(target_os = "windows") {
            println!("cargo:rustc-link-lib=static=cublas");
            println!("cargo:rustc-link-lib=static=cublasLt");
        } else {
            println!("cargo:rustc-link-lib=static=cublas_static");
            println!("cargo:rustc-link-lib=static=cublasLt_static");
            println!("cargo:rustc-link-lib=static=culibos");
        }
    }
    if cfg!(feature = "accelerate") {
        println!("cargo:rustc-link-lib=framework=Accelerate");
    }
    if cfg!(feature = "ruy") {
        println!("cargo:rustc-link-lib=cpuinfo");
        println!("cargo:rustc-link-lib=clog");
        println!("cargo:rustc-link-lib=ruy_allocator");
        println!("cargo:rustc-link-lib=ruy_apply_multiplier");
        println!("cargo:rustc-link-lib=ruy_block_map");
        println!("cargo:rustc-link-lib=ruy_blocking_counter");
        println!("cargo:rustc-link-lib=ruy_context");
        println!("cargo:rustc-link-lib=ruy_context_get_ctx");
        println!("cargo:rustc-link-lib=ruy_cpuinfo");
        println!("cargo:rustc-link-lib=ruy_ctx");
        println!("cargo:rustc-link-lib=ruy_denormal");
        println!("cargo:rustc-link-lib=ruy_frontend");
        println!("cargo:rustc-link-lib=ruy_prepacked_cache");
        println!("cargo:rustc-link-lib=ruy_prepare_packed_matrices");
        println!("cargo:rustc-link-lib=ruy_profiler_instrumentation");
        println!("cargo:rustc-link-lib=ruy_system_aligned_alloc");
        println!("cargo:rustc-link-lib=ruy_thread_pool");
        println!("cargo:rustc-link-lib=ruy_trmul");
        println!("cargo:rustc-link-lib=ruy_tune");
        println!("cargo:rustc-link-lib=ruy_wait");
        if cfg!(any(target_arch = "arm", target_arch = "aarch64")) {
            println!("cargo:rustc-link-lib=ruy_kernel_arm");
            println!("cargo:rustc-link-lib=ruy_pack_arm");
        }
        if cfg!(target_feature = "avx") {
            println!("cargo:rustc-link-lib=ruy_have_built_path_for_avx");
            println!("cargo:rustc-link-lib=ruy_kernel_avx");
            println!("cargo:rustc-link-lib=ruy_pack_avx");
        }
        if cfg!(target_feature = "avx2") {
            println!("cargo:rustc-link-lib=ruy_have_built_path_for_avx2_fma");
            println!("cargo:rustc-link-lib=ruy_kernel_avx2_fma");
            println!("cargo:rustc-link-lib=ruy_pack_avx2_fma");
        }
        if cfg!(target_feature = "avx512f") {
            println!("cargo:rustc-link-lib=ruy_have_built_path_for_avx512");
            println!("cargo:rustc-link-lib=ruy_kernel_avx512");
            println!("cargo:rustc-link-lib=ruy_pack_avx512");
        }
    }
    if cfg!(feature = "dnnl") {
        println!("cargo:rustc-link-lib=dnnl");
    }
    if cfg!(feature = "openmp-runtime-comp") {
        // See the note above: `gomp` does not exist on MSVC (LNK1181); its OpenMP
        // runtime is linked automatically via the /DEFAULTLIB:VCOMP directive.
        if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
            println!("cargo:rustc-link-lib=gomp");
        }
    }
    if cfg!(feature = "openmp-runtime-intel") {
        println!("cargo:rustc-link-lib=iomp5");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/sys");
    println!("cargo:rerun-if-changed=include");
    println!("cargo:rerun-if-changed=CTranslate2");
    add_search_paths("LIBRARY_PATH");
    println!("cargo:rerun-if-env-changed=CMAKE_INCLUDE_PATH");
    add_search_paths("CMAKE_LIBRARY_PATH");

    if cfg!(feature = "system") {
        link_system_libraries();
    } else {
        build_ctranslate2()
    }

    cxx_build::bridges([
        "src/sys/types.rs",
        "src/sys/config.rs",
        "src/sys/model_memory_reader.rs",
        "src/sys/scoring.rs",
        "src/sys/translator.rs",
        "src/sys/generator.rs",
        "src/sys/storage_view.rs",
        "src/sys/whisper.rs",
    ])
    .file("src/sys/translator.cpp")
    .file("src/sys/generator.cpp")
    .file("src/sys/whisper.cpp")
    .include("CTranslate2/include")
    .std("c++17")
    .flag_if_supported("/EHsc")
    .compile("ct2rs");
}

#[cfg(not(target_os = "windows"))]
fn is_library(name: &&str) -> bool {
    name.starts_with("lib") && name.ends_with(".a")
}

#[cfg(not(target_os = "windows"))]
fn library_name(name: &str) -> &str {
    &name[3..name.len() - 2]
}

#[cfg(target_os = "windows")]
fn is_library(name: &&str) -> bool {
    name.ends_with(".lib")
}

#[cfg(target_os = "windows")]
fn library_name(name: &str) -> &str {
    &name[0..name.len() - 4]
}

fn link_libraries<T: AsRef<Path>>(root: T) {
    let mut current_dir = None;
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            path.file_name()
                .and_then(|name| name.to_str())
                .filter(is_library)
                .iter()
                .for_each(|name| {
                    let parent = path.parent();
                    if parent != current_dir.as_deref() {
                        let dir = parent.unwrap();
                        println!("cargo:rustc-link-search={}", dir.display());
                        current_dir = Some(dir.to_path_buf())
                    }
                    println!("cargo:rustc-link-lib=static={}", library_name(name));
                });
        }
    }
}

// The function below was derived and modified from the `cudarc` crate.
// Original source: https://github.com/coreylowman/cudarc/blob/main/build.rs
//
// Copyright (c) 2024 Corey Lowman
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
fn cuda_root() -> Option<PathBuf> {
    let env_vars = [
        "CUDA_PATH",
        "CUDA_ROOT",
        "CUDA_TOOLKIT_ROOT_DIR",
        "CUDNN_LIB",
    ];
    let env_vars = env_vars
        .into_iter()
        .map(std::env::var)
        .filter_map(Result::ok);

    let roots = [
        "/usr",
        "/usr/local/cuda",
        "/opt/cuda",
        "/usr/lib/cuda",
        "C:/Program Files/NVIDIA GPU Computing Toolkit",
        "C:/CUDA",
    ];
    let roots = roots.into_iter().map(Into::into);
    env_vars
        .chain(roots)
        .map(Into::<PathBuf>::into)
        .find(|path| path.join("include").join("cuda.h").is_file())
}
