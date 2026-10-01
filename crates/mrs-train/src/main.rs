#[allow(dead_code)]
mod parent_train;
mod premise_train;
mod schedule_train;

// The rest of ENIGMA (in-loop) trainer is mostly omitted for brevity in this test to save space.
// We'll focus the CLI on invoking the new tasks.

struct TrainCfg {
    mode: String,
    epochs: usize,
    val_split: f32,
    neg_per_pos: usize,
    #[allow(dead_code)]
    inference_kind: u8,
    out_prefix: String,
}

fn parse_args() -> (String, TrainCfg) {
    let args: Vec<String> = std::env::args().collect();
    let mut mode = "premise".to_string();
    let mut epochs = 30usize;
    let mut val_split = 0.15f32;
    let mut neg_per_pos = 1usize;
    let mut inference_kind = 0u8;
    let mut positionals: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" => {
                i += 1;
                mode = args[i].clone();
            }
            "--epochs" => {
                i += 1;
                epochs = args[i].parse().expect("--epochs expects an integer");
            }
            "--val-split" => {
                i += 1;
                val_split = args[i].parse().expect("--val-split expects a float");
            }
            "--neg-per-pos" => {
                i += 1;
                neg_per_pos = args[i].parse().expect("--neg-per-pos expects an integer");
            }
            "--inference-kind" => {
                i += 1;
                inference_kind = args[i]
                    .parse::<u8>()
                    .ok()
                    .filter(|kind| *kind <= 3)
                    .unwrap_or_else(|| {
                        eprintln!("--inference-kind must be in 0..=3");
                        std::process::exit(1);
                    });
            }
            other => positionals.push(other.to_string()),
        }
        i += 1;
    }

    if positionals.is_empty() {
        eprintln!(
            "Usage: mrs-train [--mode premise|schedule|parent|enigma] [--epochs N] [--val-split F] [--neg-per-pos R] [--inference-kind 0..3] <log_dir> [out_prefix]"
        );
        std::process::exit(1);
    }
    let log_dir = positionals[0].clone();
    let out_prefix = positionals
        .get(1)
        .cloned()
        .unwrap_or_else(|| "weights".to_string());

    (
        log_dir,
        TrainCfg {
            mode,
            epochs,
            val_split,
            neg_per_pos,
            inference_kind,
            out_prefix,
        },
    )
}

fn main() {
    let (log_dir, cfg) = parse_args();

    #[cfg(feature = "cuda")]
    {
        println!("Using CUDA (LibTorch) Backend");
        type MyBackend = burn::backend::libtorch::LibTorch;
        type MyAutodiffBackend = burn::backend::autodiff::Autodiff<MyBackend>;
        let device = burn::backend::libtorch::LibTorchDevice::Cuda(0);
        if cfg.mode == "premise" {
            premise_train::train_premise::<MyAutodiffBackend>(
                device,
                &log_dir,
                &cfg.out_prefix,
                cfg.epochs,
                cfg.val_split,
                cfg.neg_per_pos,
            );
        } else if cfg.mode == "schedule" {
            schedule_train::train_schedule::<MyAutodiffBackend>(
                device,
                &log_dir,
                &cfg.out_prefix,
                cfg.epochs,
                cfg.val_split,
            );
        } else if cfg.mode == "parent" {
            #[cfg(feature = "ndarray")]
            parent_train::run(&log_dir, &cfg.out_prefix, cfg.epochs, cfg.inference_kind)
                .unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(2);
                });
            #[cfg(not(feature = "ndarray"))]
            eprintln!(
                "The parent-guidance trainer requires `--no-default-features --features ndarray`."
            );
        } else {
            println!("Old enigma training is skipped in this mode.");
        }
    }

    #[cfg(all(feature = "wgpu", not(feature = "cuda")))]
    {
        println!("Using Wgpu Backend");
        type MyBackend = burn::backend::wgpu::Wgpu;
        type MyAutodiffBackend = burn::backend::autodiff::Autodiff<MyBackend>;
        let device = burn::backend::wgpu::WgpuDevice::default();
        if cfg.mode == "premise" {
            premise_train::train_premise::<MyAutodiffBackend>(
                device,
                &log_dir,
                &cfg.out_prefix,
                cfg.epochs,
                cfg.val_split,
                cfg.neg_per_pos,
            );
        } else if cfg.mode == "schedule" {
            schedule_train::train_schedule::<MyAutodiffBackend>(
                device,
                &log_dir,
                &cfg.out_prefix,
                cfg.epochs,
                cfg.val_split,
            );
        } else if cfg.mode == "parent" {
            #[cfg(feature = "ndarray")]
            parent_train::run(&log_dir, &cfg.out_prefix, cfg.epochs, cfg.inference_kind)
                .unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(2);
                });
            #[cfg(not(feature = "ndarray"))]
            eprintln!(
                "The parent-guidance trainer requires `--no-default-features --features ndarray`."
            );
        } else {
            println!("Old enigma training is skipped in this mode.");
        }
    }

    #[cfg(all(feature = "ndarray", not(any(feature = "cuda", feature = "wgpu"))))]
    {
        println!("Using NdArray (CPU) Backend");
        type MyBackend = burn::backend::ndarray::NdArray;
        type MyAutodiffBackend = burn::backend::autodiff::Autodiff<MyBackend>;
        let device = burn::backend::ndarray::NdArrayDevice::Cpu;
        if cfg.mode == "premise" {
            premise_train::train_premise::<MyAutodiffBackend>(
                device,
                &log_dir,
                &cfg.out_prefix,
                cfg.epochs,
                cfg.val_split,
                cfg.neg_per_pos,
            );
        } else if cfg.mode == "schedule" {
            schedule_train::train_schedule::<MyAutodiffBackend>(
                device,
                &log_dir,
                &cfg.out_prefix,
                cfg.epochs,
                cfg.val_split,
            );
        } else if cfg.mode == "parent" {
            parent_train::run(&log_dir, &cfg.out_prefix, cfg.epochs, cfg.inference_kind)
                .unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(2);
                });
        } else {
            println!("Old enigma training is skipped in this mode.");
        }
    }

    #[cfg(not(any(feature = "cuda", feature = "wgpu", feature = "ndarray")))]
    {
        eprintln!("No backend feature selected. Use --features cuda | wgpu | ndarray");
    }
}
