//! Intermediate representation for expressions.
//!
//! The goal is to match wasm instructions as closely as possible, but translate
//! the stack machine into an expression tree. Additionally all control frames
//! are representd as `Block`s.

pub mod matcher;

use crate::dot::Dot;
use crate::module::functions::FunctionId;
use crate::module::functions::{DisplayExpr, DotExpr};
use crate::module::globals::GlobalId;
use crate::module::memories::MemoryId;
use crate::module::tables::TableId;
use crate::ty::TypeId;
use crate::ty::ValType;
use id_arena::Id;
use std::fmt;
use walrus_derive::walrus_expr;

/// The id of a local.
pub type LocalId = Id<Local>;

/// A local variable or parameter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Local {
    id: LocalId,
    ty: ValType,
    /// A human-readable name for this local, often useful when debugging
    pub name: Option<String>,
}

impl Local {
    /// Construct a new local from the given id and type.
    pub fn new(id: LocalId, ty: ValType) -> Local {
        Local { id, ty, name: None }
    }

    /// Get this local's id that is unique across the whole module.
    pub fn id(&self) -> LocalId {
        self.id
    }

    /// Get this local's type.
    pub fn ty(&self) -> ValType {
        self.ty
    }
}

/// An identifier for a particular expression.
pub type ExprId = Id<Expr>;

impl Dot for ExprId {
    fn dot(&self, out: &mut String) {
        out.push_str(&format!("expr_{}", self.index()))
    }
}

/// A trait for anything that is an AST node in our IR.
///
/// Implementations of this trait are generated by `#[walrus_expr]`.
pub trait Ast: Into<Expr> {
    /// The identifier type for this AST node.
    type Id: Into<ExprId>;

    /// Create a new identifier given an `ExprId` that references an `Expr` of
    /// this type.
    fn new_id(id: ExprId) -> Self::Id;
}

/// Different kinds of blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockKind {
    /// A `block` block.
    Block,

    /// A `loop` block.
    Loop,

    /// An `if` or `else` block.
    IfElse,

    /// The entry to a function.
    FunctionEntry,
}

/// An enum of all the different kinds of wasm expressions.
///
/// Note that the `#[walrus_expr]` macro rewrites this enum's variants from
///
/// ```ignore
/// enum Expr {
///     Variant { field: Ty, .. },
///     ...
/// }
/// ```
///
/// into
///
/// ```ignore
/// enum Expr {
///     Variant(Variant),
///     ...
/// }
///
/// struct Variant {
///     field: Ty,
///     ...
/// }
/// ```
#[walrus_expr]
#[derive(Clone, Debug)]
pub enum Expr {
    /// A block of multiple expressions, and also a control frame.
    #[walrus(display_name = display_block_name, dot_name = dot_block_name)]
    Block {
        /// What kind of block is this?
        #[walrus(skip_visit)] // nothing to recurse
        kind: BlockKind,
        /// The types of the expected values on the stack when entering this
        /// block.
        #[walrus(skip_visit)] // nothing to recurse
        params: Box<[ValType]>,
        /// The types of the resulting values added to the stack after this
        /// block is evaluated.
        #[walrus(skip_visit)] // nothing to recurse
        results: Box<[ValType]>,
        /// The expressions that make up the body of this block.
        exprs: Vec<ExprId>,
    },

    /// `call`
    Call {
        /// The function being invoked.
        func: FunctionId,
        /// The arguments to the function.
        args: Box<[ExprId]>,
    },

    /// `call_indirect`
    CallIndirect {
        /// The type signature of the function we're calling
        ty: TypeId,
        /// The table which `func` below is indexing into
        table: TableId,
        /// The index of the function we're invoking
        func: ExprId,
        /// The arguments to the function.
        args: Box<[ExprId]>,
    },

    /// `local.get n`
    LocalGet {
        /// The local being got.
        local: LocalId,
    },

    /// `local.set n`
    LocalSet {
        /// The local being set.
        local: LocalId,
        /// The value to set the local to.
        value: ExprId,
    },

    /// `local.tee n`
    LocalTee {
        /// The local being set.
        local: LocalId,
        /// The value to set the local to and return.
        value: ExprId,
    },

    /// `global.get n`
    GlobalGet {
        /// The global being got.
        global: GlobalId,
    },

    /// `global.set n`
    GlobalSet {
        /// The global being set.
        global: GlobalId,
        /// The value to set the global to.
        value: ExprId,
    },

    /// `*.const`
    Const {
        /// The constant value.
        value: Value,
    },

    /// Binary operations, those requiring two operands
    #[walrus(display_name = display_binop_name, dot_name = dot_binop_name)]
    Binop {
        /// The operation being performed
        #[walrus(skip_visit)]
        op: BinaryOp,
        /// The left-hand operand
        lhs: ExprId,
        /// The right-hand operand
        rhs: ExprId,
    },

    /// Unary operations, those requiring one operand
    #[walrus(display_name = display_unop_name, dot_name = dot_unop_name)]
    Unop {
        /// The operation being performed
        #[walrus(skip_visit)]
        op: UnaryOp,
        /// The input operand
        expr: ExprId,
    },

    /// `select`
    Select {
        /// The condition.
        condition: ExprId,
        /// The value returned when the condition is true. Evaluated regardless
        /// if the condition is true.
        consequent: ExprId,
        /// The value returned when the condition is false. Evaluated regardless
        /// if the condition is false.
        alternative: ExprId,
    },

    /// `unreachable`
    Unreachable {},

    /// `br`
    #[walrus(display_extra = display_br)]
    Br {
        /// The target block to branch to.
        #[walrus(skip_visit)] // should have already been visited
        block: BlockId,
        /// The arguments to the block.
        args: Box<[ExprId]>,
    },

    /// `br_if`
    #[walrus(display_extra = display_br_if)]
    BrIf {
        /// The condition for when to branch.
        condition: ExprId,
        /// The target block to branch to when the condition is met.
        #[walrus(skip_visit)] // should have already been visited
        block: BlockId,
        /// The arguments to the block.
        args: Box<[ExprId]>,
    },

    /// `if ... else ... end`
    IfElse {
        /// The condition.
        condition: ExprId,
        /// The block to execute when the condition is true.
        consequent: BlockId,
        /// The block to execute when the condition is false.
        alternative: BlockId,
    },

    /// `br_table`
    #[walrus(display_extra = display_br_table)]
    BrTable {
        /// The table index of which block to branch to.
        which: ExprId,
        /// The table of target blocks.
        #[walrus(skip_visit)] // should have already been visited
        blocks: Box<[BlockId]>,
        /// The block that is branched to by default when `which` is out of the
        /// table's bounds.
        #[walrus(skip_visit)] // should have already been visited
        default: BlockId,
        /// The arguments to the block.
        args: Box<[ExprId]>,
    },

    /// `drop`
    Drop {
        /// The expression to be evaluated and results ignored.
        expr: ExprId,
    },

    /// `return`
    Return {
        /// The values being returned.
        values: Box<[ExprId]>,
    },

    /// memory.size
    MemorySize {
        /// The memory we're fetching the current size of.
        memory: MemoryId,
    },

    /// memory.grow
    MemoryGrow {
        /// The memory we're growing.
        memory: MemoryId,
        /// The number of pages to grow by.
        pages: ExprId,
    },

    /// Loading a value from memory
    Load {
        /// The memory we're loading from.
        memory: MemoryId,
        /// The kind of memory load this is performing
        #[walrus(skip_visit)]
        kind: LoadKind,
        /// The alignment and offset of this memory load
        #[walrus(skip_visit)]
        arg: MemArg,
        /// The address that we're loading from
        address: ExprId,
    },

    /// Storing a value to memory
    Store {
        /// The memory we're storing to
        memory: MemoryId,
        /// The kind of memory store this is performing
        #[walrus(skip_visit)]
        kind: StoreKind,
        /// The alignment and offset of this memory store
        #[walrus(skip_visit)]
        arg: MemArg,
        /// The address that we're storing to
        address: ExprId,
        /// The value that we're storing
        value: ExprId,
    },
}

/// Constant values that can show up in WebAssembly
#[derive(Debug, Clone, Copy)]
pub enum Value {
    /// A constant 32-bit integer
    I32(i32),
    /// A constant 64-bit integer
    I64(i64),
    /// A constant 32-bit float
    F32(f32),
    /// A constant 64-bit float
    F64(f64),
    /// A constant 128-bit vector register
    V128(u128),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::I32(i) => i.fmt(f),
            Value::I64(i) => i.fmt(f),
            Value::F32(i) => i.fmt(f),
            Value::F64(i) => i.fmt(f),
            Value::V128(i) => i.fmt(f),
        }
    }
}

/// Possible binary operations in wasm
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug)]
pub enum BinaryOp {
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,

    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,

    F32Eq,
    F32Ne,
    F32Lt,
    F32Gt,
    F32Le,
    F32Ge,

    F64Eq,
    F64Ne,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,

    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I32Rotl,
    I32Rotr,

    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,

    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Min,
    F32Max,
    F32Copysign,

    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Min,
    F64Max,
    F64Copysign,
}

/// Possible unary operations in wasm
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug)]
pub enum UnaryOp {
    I32Eqz,
    I32Clz,
    I32Ctz,
    I32Popcnt,

    I64Eqz,
    I64Clz,
    I64Ctz,
    I64Popcnt,

    F32Abs,
    F32Neg,
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F32Sqrt,

    F64Abs,
    F64Neg,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    F64Sqrt,

    I32WrapI64,
    I32TruncSF32,
    I32TruncUF32,
    I32TruncSF64,
    I32TruncUF64,
    I64ExtendSI32,
    I64ExtendUI32,
    I64TruncSF32,
    I64TruncUF32,
    I64TruncSF64,
    I64TruncUF64,

    F32ConvertSI32,
    F32ConvertUI32,
    F32ConvertSI64,
    F32ConvertUI64,
    F32DemoteF64,
    F64ConvertSI32,
    F64ConvertUI32,
    F64ConvertSI64,
    F64ConvertUI64,
    F64PromoteF32,

    I32ReinterpretF32,
    I64ReinterpretF64,
    F32ReinterpretI32,
    F64ReinterpretI64,
}

/// The different kinds of load instructions that are part of a `Load` IR node
#[derive(Debug, Copy, Clone)]
#[allow(missing_docs)]
pub enum LoadKind {
    // TODO: much of this is probably redundant with type information already
    // ambiently available, we probably want to trim this down to just "value"
    // and then maybe some sign extensions. We'd then use the type of the node
    // to figure out what kind of store it actually is.
    I32,
    I64,
    F32,
    F64,
    V128,
    I32_8 { sign_extend: bool },
    I32_16 { sign_extend: bool },
    I64_8 { sign_extend: bool },
    I64_16 { sign_extend: bool },
    I64_32 { sign_extend: bool },
}

/// The different kinds of store instructions that are part of a `Store` IR node
#[derive(Debug, Copy, Clone)]
#[allow(missing_docs)]
pub enum StoreKind {
    I32,
    I64,
    F32,
    F64,
    V128,
    I32_8,
    I32_16,
    I64_8,
    I64_16,
    I64_32,
}

/// Arguments to memory operations, containing a constant offset from a dynamic
/// address as well as a predicted alignment.
#[derive(Debug, Copy, Clone)]
pub struct MemArg {
    /// The alignment of the memory operation, must be a power of two
    pub align: u32,
    /// The offset of the memory operation, in bytes from the source address
    pub offset: u32,
}

impl Expr {
    /// Are any instructions that follow this expression's instruction (within
    /// the current block) unreachable?
    ///
    /// Returns `true` for unconditional branches (`br`, `return`, etc...) and
    /// `unreachable`. Returns `false` for all other "normal" instructions
    /// (`i32.add`, etc...).
    pub fn following_instructions_are_unreachable(&self) -> bool {
        match *self {
            Expr::Unreachable(..) | Expr::Br(..) | Expr::BrTable(..) | Expr::Return(..) => true,

            // No `_` arm to make sure that we properly update this function as
            // we add support for new instructions.
            Expr::Block(..)
            | Expr::Call(..)
            | Expr::LocalGet(..)
            | Expr::LocalSet(..)
            | Expr::LocalTee(..)
            | Expr::GlobalGet(..)
            | Expr::GlobalSet(..)
            | Expr::Const(..)
            | Expr::Binop(..)
            | Expr::Unop(..)
            | Expr::Select(..)
            | Expr::BrIf(..)
            | Expr::IfElse(..)
            | Expr::MemorySize(..)
            | Expr::MemoryGrow(..)
            | Expr::CallIndirect(..)
            | Expr::Load(..)
            | Expr::Store(..)
            | Expr::Drop(..) => false,
        }
    }
}

impl Block {
    /// Construct a new block.
    pub fn new(kind: BlockKind, params: Box<[ValType]>, results: Box<[ValType]>) -> Block {
        let exprs = vec![];
        Block {
            kind,
            params,
            results,
            exprs,
        }
    }
}

/// Anything that can be visited by a `Visitor`.
pub trait Visit<'expr> {
    /// Visit this thing with the given visitor.
    fn visit<V>(&self, visitor: &mut V)
    where
        V: Visitor<'expr>;
}

impl<'expr> Visit<'expr> for ExprId {
    fn visit<V>(&self, visitor: &mut V)
    where
        V: Visitor<'expr>,
    {
        visitor.visit_expr(&visitor.local_function().exprs[*self])
    }
}

fn display_block_name(block: &Block, out: &mut DisplayExpr) {
    match block.kind {
        BlockKind::Loop => out.f.push_str("loop"),
        _ => out.f.push_str("block"),
    }
}

fn dot_block_name(block: &Block, out: &mut DotExpr<'_, '_>) {
    match block.kind {
        BlockKind::Loop => out.out.push_str("loop"),
        BlockKind::IfElse => out.out.push_str("if_else"),
        BlockKind::FunctionEntry => out.out.push_str("entry"),
        BlockKind::Block => out.out.push_str("block"),
    }
}

fn display_br(e: &Br, out: &mut DisplayExpr) {
    out.f
        .push_str(&format!(" (;e{};)", ExprId::from(e.block).index()))
}

fn display_br_if(e: &BrIf, out: &mut DisplayExpr) {
    out.f
        .push_str(&format!(" (;e{};)", ExprId::from(e.block).index()))
}

fn display_br_table(e: &BrTable, out: &mut DisplayExpr) {
    let blocks = e
        .blocks
        .iter()
        .map(|b| format!("e{}", ExprId::from(*b).index()))
        .collect::<Vec<_>>()
        .join(" ");
    out.f.push_str(&format!(
        " (;default:e{}  [{}];)",
        ExprId::from(e.default).index(),
        blocks
    ))
}

fn display_binop_name(e: &Binop, out: &mut DisplayExpr) {
    out.f.push_str(&format!("{:?}", e.op))
}

fn dot_binop_name(e: &Binop, out: &mut DotExpr<'_, '_>) {
    out.out.push_str(&format!("{:?}", e.op))
}

fn display_unop_name(e: &Unop, out: &mut DisplayExpr) {
    out.f.push_str(&format!("{:?}", e.op))
}

fn dot_unop_name(e: &Unop, out: &mut DotExpr<'_, '_>) {
    out.out.push_str(&format!("{:?}", e.op))
}
