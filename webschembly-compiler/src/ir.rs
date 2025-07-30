use std::fmt;

use derive_more::{From, Into};
use rustc_hash::{FxHashMap, FxHashSet};
use typed_index_collections::TiVec;

const DISPLAY_INDENT: &str = "  ";

#[derive(Debug, Clone)]
pub struct VarMeta {
    pub name: String,
}
#[derive(Debug, Clone)]
pub struct Meta {
    pub local_metas: FxHashMap<(FuncId, LocalId), VarMeta>,
    pub global_metas: FxHashMap<GlobalId, VarMeta>,
}

impl Meta {
    fn in_func(&self, func_id: FuncId) -> MetaInFunc {
        MetaInFunc {
            func_id,
            meta: self,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Display<'a, T> {
    value: T,
    meta: &'a Meta,
}

#[derive(Debug, Clone)]
pub struct DisplayInFunc<'a, T> {
    value: T,
    meta: MetaInFunc<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct MetaInFunc<'a> {
    func_id: FuncId,
    meta: &'a Meta,
}

/*
TypeもしくはmutableなTypeを表す
Type自体にMutCellを含めて再帰的にしてしまうと無限種類の型を作れるようになってしまうので、IRではそれを避けるためこのような構造になっている
TODO: LocalTypeという名前は適切ではない
*/
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum LocalType {
    #[display("mut_cell({})", _0)]
    MutCell(Type),
    #[display("{}", _0)]
    Type(Type),
}

impl From<Type> for LocalType {
    fn from(typ: Type) -> Self {
        Self::Type(typ)
    }
}

impl From<ValType> for LocalType {
    fn from(typ: ValType) -> Self {
        Self::Type(Type::from(typ))
    }
}

impl LocalType {
    pub fn to_type(&self) -> Option<Type> {
        match self {
            LocalType::Type(typ) => Some(*typ),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum Type {
    #[display("boxed")]
    Boxed,
    #[display("{}", _0)]
    Val(ValType),
}

impl From<ValType> for Type {
    fn from(typ: ValType) -> Self {
        Self::Val(typ)
    }
}

// Box化可能な型
// 基本的にSchemeの型に対応するがFuncRefなど例外もある
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy, derive_more::Display)]
pub enum ValType {
    #[display("bool")]
    Bool,
    #[display("int")]
    Int,
    #[display("string")]
    String,
    #[display("symbol")]
    Symbol,
    #[display("nil")]
    Nil,
    #[display("cons")]
    Cons,
    #[display("closure")]
    Closure,
    #[display("char")]
    Char,
    #[display("vector")]
    Vector,
    #[display("func_ref")]
    FuncRef,
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct LocalId(usize);

impl LocalId {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, LocalId> {
        DisplayInFunc { value: *self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, LocalId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "l{}", self.value.0)?;
        if let Some(meta) = self
            .meta
            .meta
            .local_metas
            .get(&(self.meta.func_id, self.value))
        {
            write!(f, "_{}", meta.name)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct GlobalId(usize);

impl GlobalId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, GlobalId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, GlobalId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "g{}", self.value.0)?;
        if let Some(meta) = self.meta.global_metas.get(&self.value) {
            write!(f, "_{}", meta.name)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct FuncId(usize);

impl FuncId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, FuncId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, FuncId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "f{}", self.value.0)?;
        // TODO: add function name

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalFlag {
    Defined,
    Used,
}

#[derive(Debug, Clone)]
pub struct ExprCall {
    pub is_tail: bool,
    pub func_id: FuncId,
    pub args: Vec<LocalId>,
}

impl ExprCall {
    pub fn modify_func_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut FuncId),
    {
        f(&mut self.func_id);
    }

    pub fn modify_local_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut LocalId),
    {
        for arg in &mut self.args {
            f(arg);
        }
    }

    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ ExprCall> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ ExprCall> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.value.is_tail {
            write!(f, "return_")?;
        }
        write!(f, "call({})", self.value.func_id.display(self.meta.meta))?;
        if !self.value.args.is_empty() {
            write!(f, "(")?;
            for (i, arg) in self.value.args.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{}", arg.display(self.meta))?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ExprCallRef {
    pub is_tail: bool,
    pub func: LocalId,
    pub args: Vec<LocalId>,
    pub func_type: FuncType,
}

impl ExprCallRef {
    pub fn modify_local_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut LocalId),
    {
        f(&mut self.func);
        for arg in &mut self.args {
            f(arg);
        }
    }

    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ ExprCallRef> {
        DisplayInFunc { value: self, meta }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ ExprCallRef> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: func_typeを表示する
        if self.value.is_tail {
            write!(f, "return_")?;
        }
        write!(f, "call_ref({})", self.value.func.display(self.meta))?;
        if !self.value.args.is_empty() {
            write!(f, "(")?;
            for (i, arg) in self.value.args.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{}", arg.display(self.meta))?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    InstantiateModule(ModuleId),
    Bool(bool),
    Int(i64),
    String(String),
    StringToSymbol(LocalId),
    Nil,
    Char(char),
    Vector(Vec<LocalId>),
    Cons(LocalId, LocalId),
    CreateMutCell(Type),
    DerefMutCell(Type, LocalId),
    SetMutCell(Type, LocalId /* mutcell */, LocalId /* value */),
    FuncRef(FuncId),
    Call(ExprCall),
    CallRef(ExprCallRef),
    Closure {
        envs: Vec<LocalId>,
        func: LocalId,
        boxed_func: LocalId,
    },
    Move(LocalId),
    Box(ValType, LocalId),
    Unbox(ValType, LocalId),
    ClosureEnv(
        Vec<LocalType>, /* env types */
        LocalId,        /* closure */
        usize,          /* env index */
    ),
    ClosureFuncRef(LocalId),
    ClosureBoxedFuncRef(LocalId),
    GlobalSet(GlobalId, LocalId),
    GlobalGet(GlobalId),
    Error(LocalId),
    InitModule,
    // builtins
    Display(LocalId),
    Add(LocalId, LocalId),
    Sub(LocalId, LocalId),
    WriteChar(LocalId),
    IsPair(LocalId),
    IsSymbol(LocalId),
    IsString(LocalId),
    IsNumber(LocalId),
    IsBoolean(LocalId),
    IsProcedure(LocalId),
    IsChar(LocalId),
    IsVector(LocalId),
    VectorLength(LocalId),
    VectorRef(LocalId, LocalId),
    VectorSet(LocalId, LocalId, LocalId),
    Eq(LocalId, LocalId),
    Not(LocalId),
    Car(LocalId),
    Cdr(LocalId),
    SymbolToString(LocalId),
    NumberToString(LocalId),
    EqNum(LocalId, LocalId),
    Lt(LocalId, LocalId),
    Gt(LocalId, LocalId),
    Le(LocalId, LocalId),
    Ge(LocalId, LocalId),
}

impl Expr {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ Expr> {
        DisplayInFunc { value: self, meta }
    }

    pub fn modify_func_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut FuncId),
    {
        match self {
            Expr::FuncRef(func_id) => f(func_id),
            Expr::Call(call) => call.modify_func_id(&mut f),
            _ => {}
        }
    }

    pub fn modify_local_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut LocalId),
    {
        match self {
            Expr::StringToSymbol(id) => f(id),
            Expr::Vector(ids) => {
                for id in ids {
                    f(id);
                }
            }
            Expr::Cons(a, b) => {
                f(a);
                f(b);
            }
            Expr::DerefMutCell(_, id) => f(id),
            Expr::SetMutCell(_, cell_id, value_id) => {
                f(cell_id);
                f(value_id);
            }
            Expr::Call(call) => {
                call.modify_local_id(&mut f);
            }
            Expr::Closure {
                envs,
                func,
                boxed_func,
            } => {
                for env in envs {
                    f(env);
                }
                f(func);
                f(boxed_func);
            }
            Expr::CallRef(call_ref) => {
                call_ref.modify_local_id(&mut f);
            }
            Expr::Move(id) => f(id),
            Expr::Box(_, id) => f(id),
            Expr::Unbox(_, id) => f(id),
            Expr::ClosureEnv(_, closure, _) => f(closure),
            Expr::ClosureFuncRef(id) => f(id),
            Expr::ClosureBoxedFuncRef(id) => f(id),
            Expr::GlobalSet(_, value) => f(value),
            Expr::Error(id) => f(id),
            Expr::Display(id) => f(id),
            Expr::Add(a, b) => {
                f(a);
                f(b);
            }
            Expr::Sub(a, b) => {
                f(a);
                f(b);
            }
            Expr::WriteChar(id) => f(id),
            Expr::IsPair(id) => f(id),
            Expr::IsSymbol(id) => f(id),
            Expr::IsString(id) => f(id),
            Expr::IsNumber(id) => f(id),
            Expr::IsBoolean(id) => f(id),
            Expr::IsProcedure(id) => f(id),
            Expr::IsChar(id) => f(id),
            Expr::IsVector(id) => f(id),
            Expr::VectorLength(id) => f(id),
            Expr::VectorRef(vec_id, index_id) => {
                f(vec_id);
                f(index_id);
            }
            Expr::VectorSet(vec_id, index_id, value_id) => {
                f(vec_id);
                f(index_id);
                f(value_id);
            }
            Expr::Eq(a, b) => {
                f(a);
                f(b);
            }
            Expr::Not(id) => f(id),
            Expr::Car(id) => f(id),
            Expr::Cdr(id) => f(id),
            Expr::SymbolToString(id) => f(id),
            Expr::NumberToString(id) => f(id),
            Expr::EqNum(a, b) => {
                f(a);
                f(b);
            }
            Expr::Lt(a, b) => {
                f(a);
                f(b);
            }
            Expr::Gt(a, b) => {
                f(a);
                f(b);
            }
            Expr::Le(a, b) => {
                f(a);
                f(b);
            }
            Expr::Ge(a, b) => {
                f(a);
                f(b);
            }

            Expr::InstantiateModule(_)
            | Expr::Bool(_)
            | Expr::Int(_)
            | Expr::String(_)
            | Expr::Nil
            | Expr::Char(_)
            | Expr::CreateMutCell(_)
            | Expr::FuncRef(_)
            | Expr::GlobalGet(_)
            | Expr::InitModule => {}
        }
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ Expr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Expr::InstantiateModule(id) => {
                write!(f, "instantiate_module({})", id.display(self.meta.meta))
            }
            Expr::Bool(b) => write!(f, "{}", b),
            Expr::Int(i) => write!(f, "{}", i),
            Expr::String(s) => write!(f, "\"{}\"", s),
            Expr::StringToSymbol(id) => write!(f, "string_to_symbol({})", id.display(self.meta)),
            Expr::Nil => write!(f, "nil"),
            Expr::Char(c) => write!(f, "'{}'", c),
            Expr::Vector(v) => {
                write!(f, "[")?;
                for (i, id) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", id.display(self.meta))?;
                }
                write!(f, "]")
            }
            Expr::Cons(a, b) => {
                write!(f, "({} . {})", a.display(self.meta), b.display(self.meta))
            }
            Expr::CreateMutCell(typ) => write!(f, "create_mut_cell<{}>", typ),
            Expr::DerefMutCell(typ, id) => {
                write!(f, "deref_mut_cell<{}>({})", typ, id.display(self.meta))
            }
            Expr::SetMutCell(typ, id, value) => {
                write!(
                    f,
                    "set_mut_cell<{}>({}, {})",
                    typ,
                    id.display(self.meta),
                    value.display(self.meta)
                )
            }
            Expr::FuncRef(id) => write!(f, "func_ref({})", id.display(self.meta.meta)),
            Expr::Call(call) => {
                write!(f, "{}", call.display(self.meta))
            }
            Expr::Closure {
                envs,
                func,
                boxed_func,
            } => {
                write!(
                    f,
                    "closure(func={}, boxed_func={}",
                    func.display(self.meta),
                    boxed_func.display(self.meta)
                )?;
                for env in envs {
                    write!(f, ", {}", env.display(self.meta))?;
                }
                write!(f, ")")
            }
            Expr::CallRef(call_ref) => {
                write!(f, "{}", call_ref.display(self.meta))
            }
            Expr::Move(id) => write!(f, "move({})", id.display(self.meta)),
            Expr::Box(typ, id) => write!(f, "box<{}>({})", typ, id.display(self.meta)),
            Expr::Unbox(typ, id) => write!(f, "unbox<{}>({})", typ, id.display(self.meta)),
            Expr::ClosureEnv(env_types, closure, index) => {
                write!(f, "closure_env<")?;
                for (i, typ) in env_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", typ)?;
                }
                write!(f, ">({}, {}]", closure.display(self.meta), index)?;
                Ok(())
            }
            Expr::ClosureFuncRef(id) => write!(f, "closure_func_ref({})", id.display(self.meta)),
            Expr::ClosureBoxedFuncRef(id) => {
                write!(f, "closure_boxed_func_ref({})", id.display(self.meta))
            }
            Expr::GlobalSet(id, value) => {
                write!(
                    f,
                    "global_set({}, {})",
                    id.display(self.meta.meta),
                    value.display(self.meta)
                )
            }
            Expr::GlobalGet(id) => write!(f, "global_get({})", id.display(self.meta.meta)),
            Expr::Error(id) => write!(f, "error({})", id.display(self.meta)),
            Expr::InitModule => {
                write!(f, "init_module")
            }
            Expr::Display(id) => write!(f, "display({})", id.display(self.meta)),
            Expr::Add(a, b) => write!(f, "add({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Sub(a, b) => write!(f, "sub({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::WriteChar(id) => write!(f, "write_char({})", id.display(self.meta)),
            Expr::IsPair(id) => write!(f, "is_pair({})", id.display(self.meta)),
            Expr::IsSymbol(id) => write!(f, "is_symbol({})", id.display(self.meta)),
            Expr::IsString(id) => write!(f, "is_string({})", id.display(self.meta)),
            Expr::IsNumber(id) => write!(f, "is_number({})", id.display(self.meta)),
            Expr::IsBoolean(id) => write!(f, "is_boolean({})", id.display(self.meta)),
            Expr::IsProcedure(id) => write!(f, "is_procedure({})", id.display(self.meta)),
            Expr::IsChar(id) => write!(f, "is_char({})", id.display(self.meta)),
            Expr::IsVector(id) => write!(f, "is_vector({})", id.display(self.meta)),
            Expr::VectorLength(id) => write!(f, "vector_length({})", id.display(self.meta)),
            Expr::VectorRef(id, index) => {
                write!(
                    f,
                    "vector_ref({}, {})",
                    id.display(self.meta),
                    index.display(self.meta)
                )
            }
            Expr::VectorSet(id, index, value) => {
                write!(
                    f,
                    "vector_set({}, {}, {})",
                    id.display(self.meta),
                    index.display(self.meta),
                    value.display(self.meta)
                )
            }
            Expr::Eq(a, b) => write!(f, "eq({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Not(id) => write!(f, "not({})", id.display(self.meta)),
            Expr::Car(id) => write!(f, "car({})", id.display(self.meta)),
            Expr::Cdr(id) => write!(f, "cdr({})", id.display(self.meta)),
            Expr::SymbolToString(id) => write!(f, "symbol_to_string({})", id.display(self.meta)),
            Expr::NumberToString(id) => write!(f, "number_to_string({})", id.display(self.meta)),
            Expr::EqNum(a, b) => write!(
                f,
                "eq_num({}, {})",
                a.display(self.meta),
                b.display(self.meta)
            ),
            Expr::Lt(a, b) => write!(f, "lt({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Gt(a, b) => write!(f, "gt({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Le(a, b) => write!(f, "le({}, {})", a.display(self.meta), b.display(self.meta)),
            Expr::Ge(a, b) => write!(f, "ge({}, {})", a.display(self.meta), b.display(self.meta)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExprAssign {
    pub local: Option<LocalId>,
    pub expr: Expr,
}

impl ExprAssign {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ ExprAssign> {
        DisplayInFunc { value: self, meta }
    }

    pub fn modify_local_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut LocalId, LocalFlag),
    {
        if let Some(local) = &mut self.local {
            f(local, LocalFlag::Defined);
        }
        self.expr.modify_local_id(|id| f(id, LocalFlag::Used));
    }
}

impl fmt::Display for DisplayInFunc<'_, &'_ ExprAssign> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(local) = self.value.local {
            write!(f, "{}", local.display(self.meta))?;
        } else {
            write!(f, "_")?;
        }
        write!(f, " = {}", self.value.expr.display(self.meta))
    }
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BasicBlockId,
    pub exprs: Vec<ExprAssign>,
    pub next: BasicBlockNext,
}

impl BasicBlock {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &'_ BasicBlock> {
        DisplayInFunc { value: self, meta }
    }

    pub fn modify_local_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut LocalId, LocalFlag),
    {
        for expr in &mut self.exprs {
            expr.modify_local_id(&mut f);
        }
        self.next.modify_local_id(|id| f(id, LocalFlag::Used));
    }

    pub fn modify_func_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut FuncId),
    {
        for expr in &mut self.exprs {
            expr.expr.modify_func_id(&mut f);
        }
        self.next.modify_func_id(&mut f);
    }
}
impl fmt::Display for DisplayInFunc<'_, &'_ BasicBlock> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // きれいな実装ではないがインデントは決め打ちする
        writeln!(
            f,
            "{}{}:",
            DISPLAY_INDENT,
            self.value.id.display(self.meta.meta)
        )?;
        for expr in &self.value.exprs {
            writeln!(
                f,
                "{}{}{}",
                DISPLAY_INDENT,
                DISPLAY_INDENT,
                expr.display(self.meta)
            )?;
        }
        writeln!(
            f,
            "{}{}{}",
            DISPLAY_INDENT,
            DISPLAY_INDENT,
            self.value.next.display(self.meta)
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct BasicBlockId(usize);

impl BasicBlockId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, BasicBlockId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, BasicBlockId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.value.0)?;
        // TODO: add basic block name
        Ok(())
    }
}

// 閉路を作ってはいけない
#[derive(Debug, Clone)]
pub enum BasicBlockNext {
    If(LocalId, BasicBlockId, BasicBlockId),
    Jump(BasicBlockId),
    Return(LocalId),
    TailCall(ExprCall),
    TailCallRef(ExprCallRef),
}

impl BasicBlockNext {
    pub fn display<'a>(&self, meta: MetaInFunc<'a>) -> DisplayInFunc<'a, &BasicBlockNext> {
        DisplayInFunc { value: self, meta }
    }

    pub fn modify_local_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut LocalId),
    {
        match self {
            BasicBlockNext::If(cond, _, _) => f(cond),
            BasicBlockNext::Jump(_) => {}
            BasicBlockNext::Return(local) => f(local),
            BasicBlockNext::TailCall(call) => call.modify_local_id(&mut f),
            BasicBlockNext::TailCallRef(call_ref) => call_ref.modify_local_id(&mut f),
        }
    }

    pub fn modify_func_id<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut FuncId),
    {
        match self {
            BasicBlockNext::If(_, _, _)
            | BasicBlockNext::Jump(_)
            | BasicBlockNext::Return(_)
            | BasicBlockNext::TailCallRef(_) => {}
            BasicBlockNext::TailCall(call) => call.modify_func_id(&mut f),
        }
    }
}

impl<'a> fmt::Display for DisplayInFunc<'_, &'a BasicBlockNext> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            BasicBlockNext::If(cond, bb1, bb2) => {
                write!(
                    f,
                    "if {} then {} else {}",
                    cond.display(self.meta),
                    bb1.display(self.meta.meta),
                    bb2.display(self.meta.meta)
                )
            }
            BasicBlockNext::Jump(bb) => write!(f, "jump {}", bb.display(self.meta.meta)),
            BasicBlockNext::Return(local) => write!(f, "return {}", local.display(self.meta)),
            BasicBlockNext::TailCall(call) => {
                write!(f, "tail_call {}", call.display(self.meta))
            }
            BasicBlockNext::TailCallRef(call_ref) => {
                write!(f, "tail_call_ref {}", call_ref.display(self.meta))
            }
        }
    }
}

impl BasicBlockNext {
    pub fn successors(&self) -> Vec<BasicBlockId> {
        match self {
            BasicBlockNext::If(_, t, f) => vec![*t, *f],
            BasicBlockNext::Jump(bb) => vec![*bb],
            BasicBlockNext::Return(_)
            | BasicBlockNext::TailCall(_)
            | BasicBlockNext::TailCallRef(_) => vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Func {
    pub id: FuncId,
    pub locals: TiVec<LocalId, LocalType>,
    // localsの先頭何個が引数か
    pub args: usize,
    pub ret_type: LocalType,
    pub bb_entry: BasicBlockId,
    pub bbs: TiVec<BasicBlockId, BasicBlock>,
}

impl Func {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, &'_ Func> {
        Display { value: self, meta }
    }

    pub fn arg_types(&self) -> Vec<LocalType> {
        (0..self.args)
            .map(|i| self.locals[LocalId::from(i)])
            .collect()
    }

    pub fn ret_type(&self) -> LocalType {
        self.ret_type
    }

    pub fn func_type(&self) -> FuncType {
        FuncType {
            args: self.arg_types(),
            ret: self.ret_type(),
        }
    }
}

impl fmt::Display for Display<'_, &'_ Func> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:", self.value.id.display(self.meta))?;
        for (local_id, local_type) in self.value.locals.iter_enumerated() {
            writeln!(
                f,
                "{}local {}: {}",
                DISPLAY_INDENT,
                local_id.display(self.meta.in_func(self.value.id)),
                local_type
            )?;
        }
        write!(f, "{}args: ", DISPLAY_INDENT)?;
        for i in 0..self.value.args {
            write!(
                f,
                "{}",
                LocalId::from(i).display(self.meta.in_func(self.value.id))
            )?;
            if i < self.value.args - 1 {
                write!(f, ",")?;
            }
        }
        writeln!(f)?;
        writeln!(f, "{}ret_type: {}", DISPLAY_INDENT, self.value.ret_type)?;
        writeln!(
            f,
            "{}entry: {}",
            DISPLAY_INDENT,
            self.value.bb_entry.display(self.meta)
        )?;
        for bb in self.value.bbs.iter() {
            write!(f, "{}", bb.display(self.meta.in_func(self.value.id)))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuncType {
    pub args: Vec<LocalType>,
    pub ret: LocalType,
}

#[derive(Debug, Clone, Copy, From, Into, Hash, PartialEq, Eq)]
pub struct ModuleId(usize);

impl ModuleId {
    pub fn display<'a>(&self, meta: &'a Meta) -> Display<'a, ModuleId> {
        Display { value: *self, meta }
    }
}

impl fmt::Display for Display<'_, ModuleId> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "module{}", self.value.0)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Module {
    pub globals: FxHashSet<GlobalId>,
    pub funcs: TiVec<FuncId, Func>,
    pub entry: FuncId,
    pub meta: Meta,
}

impl Module {
    pub fn display(&self) -> Display<'_, &Module> {
        Display {
            value: self,
            meta: &self.meta,
        }
    }
}
impl fmt::Display for Display<'_, &'_ Module> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "globals:")?;
        for (i, global) in self.value.globals.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, " {}", global.display(self.meta))?;
        }
        writeln!(f)?;
        writeln!(f, "entry: {}", self.value.entry.display(self.meta))?;
        for func in self.value.funcs.iter() {
            write!(f, "{}", func.display(self.meta))?;
        }
        Ok(())
    }
}
