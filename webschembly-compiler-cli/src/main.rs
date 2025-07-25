#![feature(path_add_extension)]

use clap::Parser;
use std::io::Write;
use std::path::Path;
use webschembly_compiler::compiler::{Compiler, Config};

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    #[arg(short, long)]
    output: Option<String>,
    #[arg(long, default_value = "false")]
    no_stdlib: bool,
    #[arg(long, default_value = "false")]
    ir: bool,
    #[arg(long, default_value = "false")]
    split_bb: bool,
    #[arg(required = true)]
    inputs: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let output = args
        .output
        .map(|s| Path::new(&s).to_path_buf())
        .unwrap_or_else(|| {
            let first_input = Path::new(args.inputs.first().unwrap());

            let ext = if args.ir { "ir" } else { "wasm" };
            first_input.with_extension(ext)
        });
    let output_stem = output.file_stem().unwrap_or_default();
    let output_extension = output.extension().unwrap_or_default();

    let mut compiler = Compiler::new(Config {
        enable_jit: false,
        enable_split_bb: args.split_bb,
    });
    let mut srcs = Vec::new();

    if !args.no_stdlib {
        srcs.push((webschembly_compiler::stdlib::generate_stdlib(), true));
    }

    for input in args.inputs {
        let src = std::fs::read_to_string(&input)?;
        srcs.push((src, false));
    }

    for (i, (src, is_stdlib)) in srcs.into_iter().enumerate() {
        let mut output = output.clone();
        output.set_file_name(output_stem);
        if i != 0 {
            output.add_extension(i.to_string());
        }

        output.add_extension(output_extension);
        let mut o = std::fs::File::create(output)?;

        if args.ir {
            let module = compiler.compile_module(&src, is_stdlib)?;
            let s = module.display().to_string();
            let bs = s.as_bytes();
            o.write_all(bs)?;
        } else {
            let bs: Vec<u8> = compiler.compile(&src, is_stdlib)?;
            o.write_all(&bs)?;
        }
    }

    Ok(())
}
