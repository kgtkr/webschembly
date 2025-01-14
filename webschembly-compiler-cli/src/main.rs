use std::io::Write;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let input = args.get(1).ok_or(anyhow::anyhow!("No input file"))?;
    let output = args.get(2).ok_or(anyhow::anyhow!("No output file"))?;
    let input = std::fs::read_to_string(input)?;
    let mut output = std::fs::File::create(output)?;

    let bs = webschembly_compiler::compile(&input)?;
    output.write_all(&bs)?;

    Ok(())
}
