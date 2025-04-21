use eyre::Context;
use log::info;
use owo_colors::OwoColorize;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TargetSpec {
    /// Examples: true, None
    #[serde(rename = "abi-return-struct-as-int")]
    pub(crate) abi_return_struct_as_int: Option<bool>,
    /// Examples: "aarch64", "x86_64"
    pub(crate) arch: String,
    /// Examples: "darwin", None
    #[serde(rename = "archive-format")]
    pub(crate) archive_format: Option<String>,
    /// Examples: "apple-m1", "x86-64"
    pub(crate) cpu: Option<String>,
    /// Examples: "false"
    #[serde(rename = "crt-objects-fallback")]
    pub(crate) crt_objects_fallback: Option<String>,
    /// Examples: true, None
    #[serde(rename = "crt-static-respected")]
    pub(crate) crt_static_respected: Option<bool>,
    /// Examples: "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32", "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
    #[serde(rename = "data-layout")]
    pub(crate) data_layout: String,
    /// Examples: "dwarf-dsym", None
    #[serde(rename = "debuginfo-kind")]
    pub(crate) debuginfo_kind: Option<String>,
    /// Examples: ".dylib", None
    #[serde(rename = "dll-suffix", default = "default_dll_suffix")]
    pub(crate) dll_suffix: String,
    /// Examples: true, None
    #[serde(rename = "dynamic-linking")]
    pub(crate) dynamic_linking: Option<bool>,
    /// Examples: "gnu", None
    pub(crate) env: Option<String>,
    /// Examples: false, None
    #[serde(rename = "eh-frame-header")]
    pub(crate) eh_frame_header: Option<bool>,
    /// Examples: false, None
    #[serde(rename = "emit-debug-gdb-scripts")]
    pub(crate) emit_debug_gdb_scripts: Option<bool>,
    /// Examples: "non-leaf", None
    #[serde(rename = "frame-pointer")]
    pub(crate) frame_pointer: Option<String>,
    /// Examples: false, None
    #[serde(rename = "function-sections")]
    pub(crate) function_sections: Option<bool>,
    /// Examples: true, None
    #[serde(rename = "has-rpath")]
    pub(crate) has_rpath: Option<bool>,
    /// Examples: true, None
    #[serde(rename = "has-thread-local")]
    pub(crate) has_thread_local: Option<bool>,
    /// Examples: true, None
    #[serde(rename = "is-like-osx")]
    pub(crate) is_like_osx: Option<bool>,
    /// Examples: ["ZERO_AR_DATE=1"], None
    #[serde(rename = "link-env")]
    pub(crate) link_env: Option<Vec<String>>,
    /// Examples: ["IPHONEOS_DEPLOYMENT_TARGET", "TVOS_DEPLOYMENT_TARGET", "XROS_DEPLOYMENT_TARGET"], None
    #[serde(rename = "link-env-remove")]
    pub(crate) link_env_remove: Option<Vec<String>>,
    /// Examples: "darwin-cc", "gnu-cc"
    #[serde(rename = "linker-flavor")]
    pub(crate) linker_flavor: String,
    /// Examples: false, None
    #[serde(rename = "linker-is-gnu")]
    pub(crate) linker_is_gnu: Option<bool>,
    /// Examples: "darwin", None
    #[serde(rename = "lld-flavor")]
    pub(crate) lld_flavor: Option<String>,
    /// Examples: "hard", None
    #[serde(rename = "llvm-floatabi")]
    pub(crate) llvm_floatabi: Option<String>,
    /// Examples: "arm64-apple-macosx", "x86_64-unknown-linux-gnu"
    #[serde(rename = "llvm-target")]
    pub(crate) llvm_target: String,
    /// Examples: 128, 64
    #[serde(rename = "max-atomic-width")]
    pub(crate) max_atomic_width: Option<u32>,
    pub(crate) metadata: Metadata,
    /// Examples: "macos", "linux"
    pub(crate) os: String,
    /// Examples: false, None
    #[serde(rename = "plt-by-default")]
    pub(crate) plt_by_default: Option<bool>,
    /// Examples: true, None
    #[serde(rename = "position-independent-executables")]
    pub(crate) position_independent_executables: Option<bool>,
    #[serde(rename = "pre-link-args")]
    pub(crate) pre_link_args: Option<PreLinkArgs>,
    /// Examples: "full", None
    #[serde(rename = "relro-level")]
    pub(crate) relro_level: Option<String>,
    /// Examples: "packed", None
    #[serde(rename = "split-debuginfo")]
    pub(crate) split_debuginfo: Option<String>,
    #[serde(rename = "stack-probes")]
    pub(crate) stack_probes: Option<StackProbes>,
    /// Examples: true, None
    #[serde(rename = "static-position-independent-executables")]
    pub(crate) static_position_independent_executables: Option<bool>,
    /// Examples: ["address", "thread", "cfi"], ["address", "leak", "memory", "thread", "cfi", "kcfi", "safestack", "dataflow"]
    #[serde(rename = "supported-sanitizers")]
    pub(crate) supported_sanitizers: Option<Vec<String>>,
    /// Examples: ["packed", "unpacked", "off"]
    #[serde(rename = "supported-split-debuginfo")]
    pub(crate) supported_split_debuginfo: Option<Vec<String>>,
    /// Examples: true, None
    #[serde(rename = "supports-xray")]
    pub(crate) supports_xray: Option<bool>,
    /// Examples: ["unix"]
    #[serde(rename = "target-family")]
    pub(crate) target_family: Option<Vec<String>>,
    /// Examples: "\u0001mcount", None
    #[serde(rename = "target-mcount")]
    pub(crate) target_mcount: Option<String>,
    /// Examples: "64"
    #[serde(rename = "target-pointer-width")]
    pub(crate) target_pointer_width: String,
    /// Examples: "apple", None
    pub(crate) vendor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct Metadata {
    /// Examples: "ARM64 Apple macOS (11.0+, Big Sur+)", "64-bit Linux (kernel 3.2+, glibc 2.17+)"
    pub(crate) description: String,
    /// Examples: true
    pub(crate) host_tools: bool,
    /// Examples: true
    pub(crate) std: bool,
    /// Examples: 1
    pub(crate) tier: u8,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct StackProbes {
    /// Examples: "inline"
    pub(crate) kind: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct PreLinkArgs {
    /// Examples: ["-m64"]
    #[serde(rename = "gnu-cc")]
    pub(crate) gnu_cc: Option<Vec<String>>,
    /// Examples: ["-m64"]
    #[serde(rename = "gnu-lld-cc")]
    pub(crate) gnu_lld_cc: Option<Vec<String>>,
}

impl TargetSpec {
    pub(crate) fn from_json(json_output: &str) -> eyre::Result<Self> {
        serde_json::from_str(json_output).wrap_err("could not deserialize target spec from JSON payload. '--print target-spec-json' is an unstable Rust flag for a reason, y'know.")
    }

    pub(crate) fn full_name(&self) -> String {
        let os = if self.os == "macos" {
            "darwin"
        } else {
            self.os.as_str()
        };
        let arch = self.arch.as_str();
        let vendor = self.vendor.as_deref().unwrap_or("unknown");
        let env = self.env.as_deref().unwrap_or("");

        if !env.is_empty() {
            format!("{}-{}-{}-{}", arch, vendor, os, env)
        } else {
            format!("{}-{}-{}", arch, vendor, os)
        }
    }

    pub(crate) fn print_info(&self) {
        // Print relevant information from TargetSpec
        info!("{}", "🎯 Target Specification:".yellow());
        let mut target_info = vec![
            format!(
                "{} {}",
                "Architecture".dimmed(),
                self.arch.cyan().underline()
            ),
            format!("{} {}", "OS".dimmed(), self.os.cyan().underline()),
        ];
        if let Some(vendor) = &self.vendor {
            target_info.push(format!(
                "{} {}",
                "Vendor".dimmed(),
                vendor.cyan().underline()
            ));
        }
        if let Some(env) = &self.env {
            target_info.push(format!(
                "{} {}",
                "Environment".dimmed(),
                env.cyan().underline()
            ));
        }
        // Comment: Basic target information
        info!("{}", target_info.join(&" :: ".dimmed().to_string()));

        let target_info = [
            format!("{} {:?}", "CPU".dimmed(), self.cpu.cyan().underline()),
            format!(
                "{} {}",
                "Pointer Width".dimmed(),
                self.target_pointer_width.cyan().underline()
            ),
            format!(
                "{} {}",
                "Dynamic Linking".dimmed(),
                self.dynamic_linking
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "???".to_string())
                    .cyan()
                    .underline()
            ),
            format!(
                "{} {}",
                "DLL Suffix".dimmed(),
                self.dll_suffix.cyan().underline()
            ),
            format!(
                "{} {}",
                "Max Atomic Width".dimmed(),
                self.max_atomic_width
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "???".to_string())
                    .cyan()
                    .underline()
            ),
        ];
        // Comment: Detailed target information
        info!("{}", target_info.join(&" :: ".dimmed().to_string()));

        let metadata_info = [
            format!("{}:", "Metadata".dimmed()),
            format!(
                "{} {}",
                "Description".dimmed(),
                self.metadata.description.cyan().underline()
            ),
            format!(
                "{} {}",
                "Host Tools".dimmed(),
                self.metadata.host_tools.to_string().cyan().underline()
            ),
            format!(
                "{} {}",
                "Standard Library".dimmed(),
                self.metadata.std.to_string().cyan().underline()
            ),
            format!(
                "{} {}",
                "Tier".dimmed(),
                self.metadata.tier.to_string().cyan().underline()
            ),
        ];
        // Comment: Metadata information
        info!("{}", metadata_info.join(&" :: ".dimmed().to_string()));
    }
}

fn default_dll_suffix() -> String {
    ".so".into()
}

/* Sample outputs:

## arm64 macOS

{
  "abi-return-struct-as-int": true,
  "arch": "aarch64",
  "archive-format": "darwin",
  "cpu": "apple-m1",
  "crt-objects-fallback": "false",
  "data-layout": "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32",
  "debuginfo-kind": "dwarf-dsym",
  "dll-suffix": ".dylib",
  "dynamic-linking": true,
  "eh-frame-header": false,
  "emit-debug-gdb-scripts": false,
  "frame-pointer": "non-leaf",
  "function-sections": false,
  "has-rpath": true,
  "has-thread-local": true,
  "is-like-osx": true,
  "link-env": [
    "ZERO_AR_DATE=1"
  ],
  "link-env-remove": [
    "IPHONEOS_DEPLOYMENT_TARGET",
    "TVOS_DEPLOYMENT_TARGET",
    "XROS_DEPLOYMENT_TARGET"
  ],
  "linker-flavor": "darwin-cc",
  "linker-is-gnu": false,
  "lld-flavor": "darwin",
  "llvm-floatabi": "hard",
  "llvm-target": "arm64-apple-macosx",
  "max-atomic-width": 128,
  "metadata": {
    "description": "ARM64 Apple macOS (11.0+, Big Sur+)",
    "host_tools": true,
    "std": true,
    "tier": 1
  },
  "os": "macos",
  "split-debuginfo": "packed",
  "stack-probes": {
    "kind": "inline"
  },
  "supported-sanitizers": [
    "address",
    "thread",
    "cfi"
  ],
  "supported-split-debuginfo": [
    "packed",
    "unpacked",
    "off"
  ],
  "target-family": [
    "unix"
  ],
  "target-mcount": "\u0001mcount",
  "target-pointer-width": "64",
  "vendor": "apple"
}

---

## x86_64 linux:

{
  "arch": "x86_64",
  "cpu": "x86-64",
  "crt-objects-fallback": "false",
  "crt-static-respected": true,
  "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
  "dynamic-linking": true,
  "env": "gnu",
  "has-rpath": true,
  "has-thread-local": true,
  "linker-flavor": "gnu-cc",
  "llvm-target": "x86_64-unknown-linux-gnu",
  "max-atomic-width": 64,
  "metadata": {
    "description": "64-bit Linux (kernel 3.2+, glibc 2.17+)",
    "host_tools": true,
    "std": true,
    "tier": 1
  },
  "os": "linux",
  "plt-by-default": false,
  "position-independent-executables": true,
  "pre-link-args": {
    "gnu-cc": [
      "-m64"
    ],
    "gnu-lld-cc": [
      "-m64"
    ]
  },
  "relro-level": "full",
  "stack-probes": {
    "kind": "inline"
  },
  "static-position-independent-executables": true,
  "supported-sanitizers": [
    "address",
    "leak",
    "memory",
    "thread",
    "cfi",
    "kcfi",
    "safestack",
    "dataflow"
  ],
  "supported-split-debuginfo": [
    "packed",
    "unpacked",
    "off"
  ],
  "supports-xray": true,
  "target-family": [
    "unix"
  ],
  "target-pointer-width": "64"
}

## x86_64 linux (docker on orbstack)

{
  "arch": "aarch64",
  "crt-objects-fallback": "false",
  "crt-static-respected": true,
  "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32",
  "dynamic-linking": true,
  "env": "gnu",
  "features": "+v8a,+outline-atomics",
  "has-rpath": true,
  "has-thread-local": true,
  "linker-flavor": "gnu-cc",
  "llvm-target": "aarch64-unknown-linux-gnu",
  "max-atomic-width": 128,
  "metadata": {
    "description": "ARM64 Linux (kernel 4.1, glibc 2.17+)",
    "host_tools": true,
    "std": true,
    "tier": 1
  },
  "os": "linux",
  "position-independent-executables": true,
  "relro-level": "full",
  "stack-probes": {
    "kind": "inline"
  },
  "supported-sanitizers": [
    "address",
    "leak",
    "memory",
    "thread",
    "hwaddress",
    "cfi",
    "memtag",
    "kcfi"
  ],
  "supported-split-debuginfo": [
    "packed",
    "unpacked",
    "off"
  ],
  "supports-xray": true,
  "target-family": [
    "unix"
  ],
  "target-mcount": "\u0001_mcount",
  "target-pointer-width": "64"
}

*/
