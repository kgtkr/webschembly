use webschembly_compiler_ir::Func;
use webschembly_compiler_ir::InstrKind;

pub fn remove_constant(func: &mut Func) {
    for local in func.locals.values_mut() {
        local.typ = local.typ.remove_constant();
    }

    func.ret_type = func.ret_type.remove_constant();
    
    if let Some(meta) = &mut func.closure_meta {
        for env_type in &mut meta.env_types {
            *env_type = env_type.remove_constant();
        }
    }

    for bb in func.bbs.values_mut() {
        for instr in &mut bb.instrs {
            match &mut instr.kind {
                InstrKind::CreateRef(typ) | InstrKind::DerefRef(typ, _) | InstrKind::SetRef(typ, _, _) => {
                    *typ = typ.remove_constant();
                }
                InstrKind::ToObj(val_type, _) | InstrKind::FromObj(val_type, _) | InstrKind::Is(val_type, _) => {
                    *val_type = val_type.remove_constant();
                }
                InstrKind::Closure { env_types, .. } | InstrKind::ClosureEnv(env_types, _, _) | InstrKind::ClosureSetEnv(env_types, _, _, _) => {
                    for env_type in env_types {
                        *env_type = env_type.remove_constant();
                    }
                }
                _ => {}
            }
        }
    }
}
