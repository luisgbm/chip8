//! The shape of a C9 program, as the parser leaves it.

/// A whole source file.
#[derive(Debug, Clone, Default)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    /// `const NAME = 12;`
    Const(Const),
    /// `var x @ V2, y @ V3;` at the top level, so it lives for the whole
    /// program.
    Var(Vec<VarDecl>),
    /// `byte name[] = { ... };` or `sprite name = { ... };`
    Data(Data),
    /// `fn name() { ... }`
    Function(Function),
}

#[derive(Debug, Clone)]
pub struct Const {
    pub name: String,
    pub value: Expr,
    pub line: usize,
}

/// One name in a `var` declaration, with the register it was pinned to if the
/// source asked for a particular one.
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub register: Option<u8>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Data {
    pub name: String,
    /// Constant expressions, one per byte.
    pub bytes: Vec<Expr>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    /// `fn add(a, b)`, each optionally pinned to a register with `@`.
    pub params: Vec<VarDecl>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    /// `var x @ V2;` inside a function.
    Var(Vec<VarDecl>),
    /// `name:`
    Label(String),
    /// `goto name;`
    Goto(String),
    /// `return;` or `return value;`
    Return(Option<Expr>),
    /// `break;` and `continue;`, which need the enclosing loop.
    Break,
    Continue,
    /// `{ ... }`
    Block(Vec<Stmt>),
    /// `if (cond) then else otherwise`
    If {
        condition: Expr,
        then: Box<Stmt>,
        otherwise: Option<Box<Stmt>>,
    },
    /// `while (cond) body`
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    /// `do body while (cond);`
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
    },
    /// `loop body`, which never ends on its own.
    Loop(Box<Stmt>),
    /// `target op= value;`
    Assign {
        target: Place,
        operator: AssignOp,
        value: Expr,
    },
    /// A call on its own: `draw(x, y, player);`
    Call {
        name: String,
        arguments: Vec<Expr>,
    },
}

/// Somewhere a value can be put.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// A variable, so one of `V0` to `VF`.
    Var(String),
    /// The delay timer.
    Delay,
    /// The sound timer, which cannot be read back.
    Sound,
    /// The address register, which holds a sprite or an array address.
    Index,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Shr,
    Shl,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Number(i32),
    /// A variable, a constant, or the name of some data.
    Name(String),
    /// `V0` .. `VF` written out, which `store` and `restore` need because they
    /// work on a run of registers rather than on one named value.
    Register(u8),
    /// `delay`, which is the only readable special register.
    Delay,
    Binary {
        operator: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `!cond`
    Not(Box<Expr>),
    /// `a && b` and `a || b`, which only work out the right hand side when the
    /// left hand one has not already settled the answer.
    Logical {
        operator: LogicalOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `name[index]`
    Index {
        name: String,
        index: Box<Expr>,
    },
    /// `random(0xFF)`, `key()`, `font(x)`, `pressed(k)`
    Call {
        name: String,
        arguments: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Shr,
    Shl,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl BinOp {
    /// Comparisons produce a branch rather than a value, so they are only
    /// allowed where a condition is expected.
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Equal
                | Self::NotEqual
                | Self::Less
                | Self::LessOrEqual
                | Self::Greater
                | Self::GreaterOrEqual
        )
    }

    /// Reads the comparison the other way round, which is how `a > b` becomes
    /// `b < a` and lets the code generator deal with only `<` and `>=`.
    pub fn swapped(self) -> Self {
        match self {
            Self::Less => Self::Greater,
            Self::Greater => Self::Less,
            Self::LessOrEqual => Self::GreaterOrEqual,
            Self::GreaterOrEqual => Self::LessOrEqual,
            other => other,
        }
    }

    /// The comparison that is true exactly when this one is false.
    pub fn negated(self) -> Self {
        match self {
            Self::Equal => Self::NotEqual,
            Self::NotEqual => Self::Equal,
            Self::Less => Self::GreaterOrEqual,
            Self::GreaterOrEqual => Self::Less,
            Self::LessOrEqual => Self::Greater,
            Self::Greater => Self::LessOrEqual,
            other => other,
        }
    }
}
