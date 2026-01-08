use webschembly_compiler_ir::Func;
use webschembly_compiler_ir::InstrKind;

pub fn remove_constant(func: &mut Func) {
    for local in func.locals.values_mut() {
        local.typ = local.typ.remove_constant();
    }

    func.ret_type = func.ret_type.remove_constant();
}
