#![feature(string_deref_patterns, box_patterns)]
pub mod lexer;
pub mod parser_combinator;
#[macro_use]
pub mod sexpr;
pub mod ast;
pub mod codegen;
pub mod ir;
pub mod sexpr_parser;
pub mod token;

pub fn compile(input: &str) -> anyhow::Result<Vec<u8>> {
    let (_, tokens) = lexer::tokens(input).map_err(|e| anyhow::anyhow!("{}", e))?;
    let (_, sexpr) =
        sexpr_parser::sexpr(tokens.as_slice()).map_err(|e| anyhow::anyhow!("{}", e))?;
    let ast = ast::AST::from_sexpr(sexpr)?;
    let ir = ir::Ir::from_ast(&ast)?;
    let code = codegen::ModuleGenerator::new().gen(&ir);
    Ok(code.finish())
}
