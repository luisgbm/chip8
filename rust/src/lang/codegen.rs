//! Turns the tree from [`super::ast`] into assembly text.
//!
//! # How the machine is used
//!
//! CHIP-8 has sixteen registers and no stack for values, so the register
//! layout is fixed and simple:
//!
//! * `V0` is the accumulator. Every value the compiler works out lands here.
//! * `V1` is a staging slot for the right hand side of a two register
//!   instruction. Its contents are deliberately *not* tracked, so it is
//!   written afresh every time it is needed.
//! * `VF` is the flag, written by the machine itself.
//! * Everything else belongs to your variables.
//!
//! `V0` and `I` are tracked across the whole program by
//! [`analyse`], so a value that is already in place is not loaded again. That
//! is what turns
//!
//! ```text
//! if (x < HOLE_MIN) return;
//! if (x - HOLE_MIN >= HOLE_W) return;
//! ```
//!
//! into a single subtraction followed by a second one that reuses it.

use std::collections::HashMap;
use std::fmt::Write as _;

use super::ast::{
    AssignOp, BinOp, Data, Expr, ExprKind, Function, Item, Place, Program, Stmt, StmtKind, VarDecl,
};
use super::CompileError;

/// The accumulator, which holds every value the compiler works out.
const ACC: u8 = 0;
/// The staging slot for the right hand operand.
const STAGE: u8 = 1;
/// The flag register, which belongs to the machine.
const FLAG: u8 = 15;

/// Every glyph in the interpreter's built in font is this tall.
const FONT_HEIGHT: u8 = 5;

/// What an instruction does to a register the compiler is tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Effect {
    /// Leaves it alone.
    Keep,
    /// Puts something in it that is not worth tracking.
    Clobber,
    /// Puts a known value in it.
    Set(String),
}

/// Where control can go after an instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Flow {
    /// On to the next one.
    Next,
    /// On to the next one, or the one after it.
    Skip,
    /// Only to a label.
    Jump(String),
    /// To a subroutine and back.
    Call,
    /// Back to whoever called this function.
    Return,
}

#[derive(Debug, Clone)]
struct Instruction {
    /// Labels sitting in front of it, in source order.
    labels: Vec<String>,
    text: String,
    flow: Flow,
    accumulator: Effect,
    index_register: Effect,
    reads_flag: bool,
    writes_flag: bool,
}

impl Instruction {
    fn new(text: impl Into<String>) -> Self {
        Self {
            labels: Vec::new(),
            text: text.into(),
            flow: Flow::Next,
            accumulator: Effect::Keep,
            index_register: Effect::Keep,
            reads_flag: false,
            writes_flag: false,
        }
    }
}

/// A run of instructions that puts one known value in one register, and so can
/// be dropped when that value is already there.
#[derive(Debug, Clone)]
struct Group {
    start: usize,
    end: usize,
    value: String,
    /// Whether it is the accumulator being loaded, or `I`.
    accumulator: bool,
    /// Whether dropping it would leave a stale flag behind.
    disturbs_flag: bool,
}

/// What the compiler knows about a register at some point in the program.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Known {
    /// Not worked out yet. Only ever seen while the analysis is running.
    Unvisited,
    Value(String),
    Anything,
}

impl Known {
    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Unvisited, value) | (value, Self::Unvisited) => value.clone(),
            (Self::Value(left), Self::Value(right)) if left == right => self.clone(),
            _ => Self::Anything,
        }
    }

    fn after(&self, effect: &Effect) -> Self {
        match effect {
            Effect::Keep => self.clone(),
            Effect::Clobber => Self::Anything,
            Effect::Set(value) => Self::Value(value.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    accumulator: Known,
    index_register: Known,
}

struct Loop {
    top: String,
    end: String,
    end_used: bool,
}

pub struct Compiler {
    constants: HashMap<String, i32>,
    variables: HashMap<String, u8>,
    data: HashMap<String, usize>,
    functions: Vec<String>,
    code: Vec<Instruction>,
    groups: Vec<Group>,
    /// Labels waiting to be attached to the next instruction.
    pending: Vec<String>,
    loops: Vec<Loop>,
    next_label: usize,
    /// Registers already spoken for, so automatic ones do not collide.
    taken: [bool; 16],
    line: usize,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    #[must_use]
    pub fn new() -> Self {
        let mut taken = [false; 16];
        taken[ACC as usize] = true;
        taken[STAGE as usize] = true;
        taken[FLAG as usize] = true;

        Self {
            constants: HashMap::new(),
            variables: HashMap::new(),
            data: HashMap::new(),
            functions: Vec::new(),
            code: Vec::new(),
            groups: Vec::new(),
            pending: Vec::new(),
            loops: Vec::new(),
            next_label: 0,
            taken,
            line: 0,
        }
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T, CompileError> {
        Err(CompileError {
            line: self.line,
            message: message.into(),
        })
    }

    // -- emitting ------------------------------------------------------------

    fn push(&mut self, mut instruction: Instruction) -> usize {
        instruction.labels = std::mem::take(&mut self.pending);
        self.code.push(instruction);
        self.code.len() - 1
    }

    fn emit(&mut self, text: impl Into<String>) -> usize {
        self.push(Instruction::new(text))
    }

    /// Puts a label here. If something else has already claimed this spot the
    /// existing name is reused, which is what keeps a loop that the source
    /// labelled from growing a second, generated name.
    fn place_label(&mut self, name: String) {
        self.pending.push(name);
    }

    fn label_here(&mut self) -> String {
        if let Some(name) = self.pending.first() {
            return name.clone();
        }

        let name = format!("_L{}", self.next_label);
        self.next_label += 1;
        self.pending.push(name.clone());
        name
    }

    fn fresh_label(&mut self) -> String {
        let name = format!("_L{}", self.next_label);
        self.next_label += 1;
        name
    }

    fn jump(&mut self, target: &str) {
        let mut instruction = Instruction::new(format!("JP {target}"));
        instruction.flow = Flow::Jump(target.to_owned());
        self.push(instruction);
    }

    // -- names ---------------------------------------------------------------

    fn constant(&self, expr: &Expr) -> Option<i32> {
        match &expr.kind {
            ExprKind::Number(value) => Some(*value),
            ExprKind::Name(name) => self.constants.get(name).copied(),
            ExprKind::Binary {
                operator,
                left,
                right,
            } => {
                let left = self.constant(left)?;
                let right = self.constant(right)?;
                match operator {
                    BinOp::Add => Some(left + right),
                    BinOp::Sub => Some(left - right),
                    BinOp::And => Some(left & right),
                    BinOp::Or => Some(left | right),
                    BinOp::Xor => Some(left ^ right),
                    BinOp::Shl => Some(left << right),
                    BinOp::Shr => Some(left >> right),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn register_of(&self, name: &str) -> Option<u8> {
        self.variables.get(name).copied()
    }

    /// A short name for a value, used to tell whether the accumulator already
    /// holds it. Constants are folded first so that `ZERO_VY` and `8` are the
    /// same thing.
    fn value_key(&self, expr: &Expr) -> Option<String> {
        if let Some(value) = self.constant(expr) {
            return Some(format!("#{value}"));
        }

        match &expr.kind {
            ExprKind::Name(name) => self.register_of(name).map(|_| name.clone()),
            ExprKind::Register(index) => Some(format!("V{index:X}")),
            ExprKind::Binary {
                operator,
                left,
                right,
            } => {
                let left = self.value_key(left)?;
                let right = self.value_key(right)?;
                Some(format!("({left}{operator:?}{right})"))
            }
            _ => None,
        }
    }

    // -- the program ---------------------------------------------------------

    pub fn compile(&mut self, program: &Program) -> Result<String, CompileError> {
        self.collect(program)?;

        let functions: Vec<&Function> = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .collect();

        if functions.is_empty() {
            return Err(CompileError {
                line: 1,
                message:
                    "a program needs at least one function, and the first one is where it starts"
                        .into(),
            });
        }

        for function in &functions {
            self.line = function.line;
            self.place_label(function.name.clone());
            self.function(function)?;
        }

        if !self.pending.is_empty() {
            // A label with nothing after it still needs somewhere to sit.
            self.emit("RET");
        }

        self.analyse();

        let data: Vec<&Data> = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Data(data) => Some(data),
                _ => None,
            })
            .collect();

        self.render(&data)
    }

    fn collect(&mut self, program: &Program) -> Result<(), CompileError> {
        // Constants first, in order, so one may be built out of another.
        for item in &program.items {
            if let Item::Const(constant) = item {
                self.line = constant.line;
                let Some(value) = self.constant(&constant.value) else {
                    return self.error(format!(
                        "`{}` has to be a constant, so it cannot depend on a variable",
                        constant.name
                    ));
                };
                if self
                    .constants
                    .insert(constant.name.clone(), value)
                    .is_some()
                {
                    return self.error(format!("`{}` is defined twice", constant.name));
                }
            }
        }

        for item in &program.items {
            match item {
                Item::Data(data) => {
                    self.line = data.line;
                    if data.bytes.is_empty() {
                        return self.error(format!("`{}` has no bytes in it", data.name));
                    }
                    if self
                        .data
                        .insert(data.name.clone(), data.bytes.len())
                        .is_some()
                    {
                        return self.error(format!("`{}` is defined twice", data.name));
                    }
                }
                Item::Function(function) => {
                    self.line = function.line;
                    if self.functions.contains(&function.name) {
                        return self.error(format!("`{}` is defined twice", function.name));
                    }
                    self.functions.push(function.name.clone());
                }
                Item::Var(declarations) => {
                    for declaration in declarations {
                        self.declare(declaration)?;
                    }
                }
                Item::Const(_) => {}
            }
        }

        Ok(())
    }

    fn declare(&mut self, declaration: &VarDecl) -> Result<(), CompileError> {
        self.line = declaration.line;

        if self.variables.contains_key(&declaration.name) {
            return self.error(format!("`{}` is declared twice", declaration.name));
        }
        if self.constants.contains_key(&declaration.name) {
            return self.error(format!("`{}` is already a constant", declaration.name));
        }

        let register = match declaration.register {
            Some(register) => register,
            None => {
                let Some(free) = (0..16).find(|index| !self.taken[*index as usize]) else {
                    return self.error("there are no registers left");
                };
                free
            }
        };

        if declaration.register.is_none() {
            self.taken[register as usize] = true;
        }

        self.variables.insert(declaration.name.clone(), register);
        Ok(())
    }

    fn function(&mut self, function: &Function) -> Result<(), CompileError> {
        let start = self.code.len();
        self.block(&function.body)?;

        // A function that has already jumped away has nowhere to return from,
        // but a jump sitting under a skip is one execution steps straight past.
        let last = self.code.len().checked_sub(1);
        let ends_in_jump = self.pending.is_empty()
            && last.is_some_and(|last| {
                last >= start
                    && matches!(self.code[last].flow, Flow::Jump(_))
                    && (last == start || self.code[last - 1].flow != Flow::Skip)
            });

        if !ends_in_jump {
            let mut instruction = Instruction::new("RET");
            instruction.flow = Flow::Return;
            self.push(instruction);
        }

        Ok(())
    }

    fn block(&mut self, body: &[Stmt]) -> Result<(), CompileError> {
        for statement in body {
            self.statement(statement)?;
        }
        Ok(())
    }

    // -- statements ----------------------------------------------------------

    fn statement(&mut self, statement: &Stmt) -> Result<(), CompileError> {
        self.line = statement.line;

        match &statement.kind {
            StmtKind::Var(declarations) => {
                for declaration in declarations {
                    self.declare(declaration)?;
                }
            }
            StmtKind::Label(name) => self.place_label(name.clone()),
            StmtKind::Goto(name) => self.jump(&name.clone()),
            StmtKind::Return => {
                let mut instruction = Instruction::new("RET");
                instruction.flow = Flow::Return;
                self.push(instruction);
            }
            StmtKind::Break => {
                let Some(enclosing) = self.loops.last_mut() else {
                    return self.error("`break` is only allowed inside a loop");
                };
                enclosing.end_used = true;
                let target = enclosing.end.clone();
                self.jump(&target);
            }
            StmtKind::Continue => {
                let Some(enclosing) = self.loops.last() else {
                    return self.error("`continue` is only allowed inside a loop");
                };
                let target = enclosing.top.clone();
                self.jump(&target);
            }
            StmtKind::Block(body) => self.block(body)?,
            StmtKind::If {
                condition,
                then,
                otherwise,
            } => self.compile_if(condition, then, otherwise.as_deref())?,
            StmtKind::While { condition, body } => self.compile_while(condition, body)?,
            StmtKind::DoWhile { body, condition } => self.compile_do_while(body, condition)?,
            StmtKind::Loop(body) => self.compile_loop(body)?,
            StmtKind::Assign {
                target,
                operator,
                value,
            } => self.assign(target, *operator, value)?,
            StmtKind::Call { name, arguments } => self.call(name, arguments)?,
        }

        Ok(())
    }

    /// Everything needed to undo a trial run of some statements.
    fn mark(&self) -> (usize, usize, Vec<String>, usize) {
        (
            self.code.len(),
            self.groups.len(),
            self.pending.clone(),
            self.next_label,
        )
    }

    fn restore(&mut self, mark: (usize, usize, Vec<String>, usize)) {
        let (code, groups, pending, next_label) = mark;
        self.code.truncate(code);
        self.groups.truncate(groups);
        self.pending = pending;
        self.next_label = next_label;
    }

    fn compile_if(
        &mut self,
        condition: &Expr,
        then: &Stmt,
        otherwise: Option<&Stmt>,
    ) -> Result<(), CompileError> {
        let test = self.condition(condition)?;

        // A body of exactly one instruction can hang straight off the skip,
        // which is what the machine's skip instructions are for.
        if otherwise.is_none() {
            let mark = self.mark();
            self.statement(then)?;

            // Labels waiting to be placed will land on the skip instead, so
            // only a label made inside the body rules this out.
            let single = self.code.len() == mark.0 + 1
                && self.pending.is_empty()
                && self.code[mark.0].labels == mark.2;

            self.restore(mark);

            if single {
                self.emit_skip(&test, false);
                self.statement(then)?;
                return Ok(());
            }
        }

        let otherwise_label = self.fresh_label();
        self.emit_skip(&test, true);
        self.jump(&otherwise_label);
        self.statement(then)?;

        if let Some(otherwise) = otherwise {
            let end = self.fresh_label();
            self.jump(&end);
            self.place_label(otherwise_label);
            self.statement(otherwise)?;
            self.place_label(end);
        } else {
            self.place_label(otherwise_label);
        }

        Ok(())
    }

    fn compile_while(&mut self, condition: &Expr, body: &Stmt) -> Result<(), CompileError> {
        let top = self.label_here();
        let end = self.fresh_label();

        let test = self.condition(condition)?;
        self.emit_skip(&test, true);
        self.jump(&end);

        self.loops.push(Loop {
            top: top.clone(),
            end: end.clone(),
            end_used: true,
        });
        self.statement(body)?;
        self.loops.pop();

        self.jump(&top);
        self.place_label(end);

        Ok(())
    }

    fn compile_do_while(&mut self, body: &Stmt, condition: &Expr) -> Result<(), CompileError> {
        let top = self.label_here();
        let end = self.fresh_label();

        self.loops.push(Loop {
            top: top.clone(),
            end: end.clone(),
            end_used: false,
        });
        self.statement(body)?;

        let test = self.condition(condition)?;
        self.emit_skip(&test, false);
        self.jump(&top);

        let used = self.loops.pop().is_some_and(|enclosing| enclosing.end_used);
        if used {
            self.place_label(end);
        }

        Ok(())
    }

    fn compile_loop(&mut self, body: &Stmt) -> Result<(), CompileError> {
        let top = self.label_here();
        let end = self.fresh_label();

        self.loops.push(Loop {
            top: top.clone(),
            end: end.clone(),
            end_used: false,
        });
        self.statement(body)?;
        let used = self.loops.pop().is_some_and(|enclosing| enclosing.end_used);

        self.jump(&top);

        if used {
            self.place_label(end);
        }

        Ok(())
    }

    // -- conditions ----------------------------------------------------------

    /// Works out the value a condition turns on, emitting whatever has to be
    /// computed first, and returns the test the skip instruction will make.
    fn condition(&mut self, condition: &Expr) -> Result<Test, CompileError> {
        self.line = condition.line;

        if let ExprKind::Not(inner) = &condition.kind {
            let mut test = self.condition(inner)?;
            test.negated = !test.negated;
            return Ok(test);
        }

        if let ExprKind::Call { name, arguments } = &condition.kind {
            if name == "pressed" {
                let [key] = arguments.as_slice() else {
                    return self.error("`pressed` takes one key");
                };
                let register = self.register_argument(key)?;
                return Ok(Test {
                    kind: TestKind::KeyDown(register),
                    negated: false,
                });
            }
        }

        let ExprKind::Binary {
            operator,
            left,
            right,
        } = &condition.kind
        else {
            return self.error(
                "a condition has to be a comparison, `pressed(key)`, or one of those with `!` in front",
            );
        };

        if !operator.is_comparison() {
            return self.error(format!(
                "`{operator:?}` produces a value, not a condition, so it needs a comparison around it"
            ));
        }

        // `a > b` is `b < a`, which halves the cases below.
        let (operator, left, right) = match operator {
            BinOp::Greater | BinOp::LessOrEqual => (operator.swapped(), right, left),
            _ => (*operator, left, right),
        };

        if matches!(operator, BinOp::Equal | BinOp::NotEqual) {
            let register = self.register_argument(left)?;
            let operand = if let Some(value) = self.constant(right) {
                Operand::Immediate(self.byte(value)?)
            } else {
                Operand::Register(self.register_argument(right)?)
            };

            return Ok(Test {
                kind: TestKind::Equal { register, operand },
                negated: operator == BinOp::NotEqual,
            });
        }

        // Everything else is a subtraction read through the flag: `VF` comes
        // out 1 when the left side was the larger, so `a >= b`.
        let difference = Expr {
            kind: ExprKind::Binary {
                operator: BinOp::Sub,
                left: Box::new(left.as_ref().clone()),
                right: Box::new(right.as_ref().clone()),
            },
            line: condition.line,
        };

        self.eval_accumulator(&difference, true)?;

        Ok(Test {
            kind: TestKind::NoBorrow,
            negated: operator == BinOp::Less,
        })
    }

    /// Emits the skip in front of a one instruction body. `skip_when` says
    /// whether to skip while the condition holds or while it does not.
    fn emit_skip(&mut self, test: &Test, skip_when: bool) {
        let skip_when_true = skip_when != test.negated;

        let (text, reads_flag) = match &test.kind {
            TestKind::Equal { register, operand } => {
                let mnemonic = if skip_when_true { "SE" } else { "SNE" };
                let text = match operand {
                    Operand::Immediate(value) => {
                        format!("{mnemonic} V{register:X}, {value}")
                    }
                    Operand::Register(other) => format!("{mnemonic} V{register:X}, V{other:X}"),
                };
                (text, false)
            }
            TestKind::KeyDown(register) => {
                let mnemonic = if skip_when_true { "SKP" } else { "SKNP" };
                (format!("{mnemonic} V{register:X}"), false)
            }
            TestKind::NoBorrow => {
                let wanted = u8::from(skip_when_true);
                (format!("SE V{FLAG:X}, {wanted}"), true)
            }
        };

        let mut instruction = Instruction::new(text);
        instruction.flow = Flow::Skip;
        instruction.reads_flag = reads_flag;
        self.push(instruction);
    }

    // -- assignment ----------------------------------------------------------

    fn assign(
        &mut self,
        target: &Place,
        operator: AssignOp,
        value: &Expr,
    ) -> Result<(), CompileError> {
        match target {
            Place::Var(name) => self.assign_variable(name, operator, value),
            Place::Delay | Place::Sound => {
                if operator != AssignOp::Set {
                    return self.error("a timer can only be set, not changed in place");
                }

                let register = if let Some(register) = self.register_named(value) {
                    register
                } else {
                    self.eval_accumulator(value, false)?;
                    ACC
                };

                let timer = if matches!(target, Place::Delay) {
                    "DT"
                } else {
                    "ST"
                };
                self.emit(format!("LD {timer}, V{register:X}"));
                Ok(())
            }
            Place::Index => self.assign_index(operator, value),
        }
    }

    fn assign_index(&mut self, operator: AssignOp, value: &Expr) -> Result<(), CompileError> {
        match operator {
            AssignOp::Set => {
                let ExprKind::Name(name) = &value.kind else {
                    return self.error("`I` can only be set to the name of a sprite or an array");
                };
                if !self.data.contains_key(name) {
                    return self.error(format!("`{name}` is not a sprite or an array"));
                }
                self.load_index(name);
                Ok(())
            }
            AssignOp::Add => {
                let register = self.register_argument(value)?;
                let mut instruction = Instruction::new(format!("ADD I, V{register:X}"));
                instruction.index_register = Effect::Clobber;
                self.push(instruction);
                Ok(())
            }
            _ => self.error("`I` can only be set or added to"),
        }
    }

    fn assign_variable(
        &mut self,
        name: &str,
        operator: AssignOp,
        value: &Expr,
    ) -> Result<(), CompileError> {
        let destination = self.lookup_variable(name)?;

        // `x >>= 1` and `x = y >> 1` both shift in place, which is the only
        // shift the machine has.
        if operator == AssignOp::Shr || operator == AssignOp::Shl {
            let amount = self.constant(value);
            if amount != Some(1) {
                return self.error("the machine can only shift by one");
            }
            return self.shift(destination, operator == AssignOp::Shr, None);
        }

        if operator == AssignOp::Set {
            if let ExprKind::Binary {
                operator: BinOp::Shr | BinOp::Shl,
                left,
                right,
            } = &value.kind
            {
                if self.constant(right) != Some(1) {
                    return self.error("the machine can only shift by one");
                }
                let shift_right = matches!(
                    value.kind,
                    ExprKind::Binary {
                        operator: BinOp::Shr,
                        ..
                    }
                );
                return self.shift(destination, shift_right, Some(left));
            }

            // A plain value goes straight in.
            if let Some(number) = self.constant(value) {
                let byte = self.byte(number)?;
                self.load_immediate(destination, byte);
                return Ok(());
            }

            if let ExprKind::Name(source) = &value.kind {
                let source = self.lookup_variable(source)?;
                if source != destination {
                    self.move_register(destination, source);
                }
                return Ok(());
            }

            if matches!(value.kind, ExprKind::Delay) {
                let mut instruction = Instruction::new(format!("LD V{destination:X}, DT"));
                if destination == ACC {
                    instruction.accumulator = Effect::Clobber;
                }
                self.push(instruction);
                return Ok(());
            }

            // `x = x + 1` is the same instruction as `x += 1`.
            if let ExprKind::Binary {
                operator: inner,
                left,
                right,
            } = &value.kind
            {
                if let ExprKind::Name(left_name) = &left.kind {
                    if self.register_of(left_name) == Some(destination) {
                        if let Some(assign) = binary_to_assign(*inner) {
                            return self.assign_variable(name, assign, right);
                        }
                    }
                }
            }

            self.eval_accumulator(value, false)?;
            if destination != ACC {
                self.move_register(destination, ACC);
            }
            return Ok(());
        }

        // The rest are the read-modify-write forms.
        let mnemonic = match operator {
            AssignOp::Add => "ADD",
            AssignOp::Sub => "SUB",
            AssignOp::And => "AND",
            AssignOp::Or => "OR",
            AssignOp::Xor => "XOR",
            AssignOp::Set | AssignOp::Shr | AssignOp::Shl => unreachable!(),
        };

        // Only `ADD` has an immediate form, and it is the one that does not
        // touch the flag.
        if operator == AssignOp::Add {
            if let Some(number) = self.constant(value) {
                let byte = self.byte(number)?;
                let mut instruction = Instruction::new(format!("ADD V{destination:X}, {byte}"));
                if destination == ACC {
                    instruction.accumulator = Effect::Clobber;
                }
                self.push(instruction);
                return Ok(());
            }
        }

        let source = if let Some(register) = self.register_named(value) {
            register
        } else {
            self.eval_accumulator(value, false)?;
            ACC
        };

        let mut instruction = Instruction::new(format!("{mnemonic} V{destination:X}, V{source:X}"));
        instruction.writes_flag = true;
        if destination == ACC {
            instruction.accumulator = Effect::Clobber;
        }
        self.push(instruction);

        Ok(())
    }

    fn shift(
        &mut self,
        destination: u8,
        right: bool,
        source: Option<&Expr>,
    ) -> Result<(), CompileError> {
        if let Some(source) = source {
            let source = self.register_argument(source)?;
            if source != destination {
                self.move_register(destination, source);
            }
        }

        let mnemonic = if right { "SHR" } else { "SHL" };
        let mut instruction = Instruction::new(format!("{mnemonic} V{destination:X}"));
        instruction.writes_flag = true;
        if destination == ACC {
            instruction.accumulator = Effect::Clobber;
        }
        self.push(instruction);

        Ok(())
    }
}

/// The comparison a skip instruction makes.
struct Test {
    kind: TestKind,
    negated: bool,
}

enum TestKind {
    Equal {
        register: u8,
        operand: Operand,
    },
    KeyDown(u8),
    /// The flag after a subtraction, which is 1 when there was no borrow.
    NoBorrow,
}

enum Operand {
    Immediate(u8),
    Register(u8),
}

fn binary_to_assign(operator: BinOp) -> Option<AssignOp> {
    match operator {
        BinOp::Add => Some(AssignOp::Add),
        BinOp::Sub => Some(AssignOp::Sub),
        BinOp::And => Some(AssignOp::And),
        BinOp::Or => Some(AssignOp::Or),
        BinOp::Xor => Some(AssignOp::Xor),
        _ => None,
    }
}

impl Compiler {
    // -- values --------------------------------------------------------------

    fn byte(&self, value: i32) -> Result<u8, CompileError> {
        u8::try_from(value).map_or_else(
            |_| self.error(format!("{value} does not fit in a byte")),
            Ok,
        )
    }

    fn lookup_variable(&self, name: &str) -> Result<u8, CompileError> {
        self.register_of(name).map_or_else(
            || {
                if self.constants.contains_key(name) {
                    self.error(format!(
                        "`{name}` is a constant, so it cannot be assigned to"
                    ))
                } else {
                    self.error(format!("`{name}` has not been declared"))
                }
            },
            Ok,
        )
    }

    /// The register an expression already lives in, when it is simply the name
    /// of a variable or a register written out. Constants do not count: they
    /// have to be loaded.
    fn register_named(&self, expr: &Expr) -> Option<u8> {
        match &expr.kind {
            ExprKind::Name(name) => self.register_of(name),
            ExprKind::Register(index) => Some(*index),
            _ => None,
        }
    }

    fn load_immediate(&mut self, register: u8, value: u8) {
        let mut instruction = Instruction::new(format!("LD V{register:X}, {value}"));
        if register == ACC {
            instruction.accumulator = Effect::Clobber;
        }
        self.push(instruction);
    }

    fn move_register(&mut self, destination: u8, source: u8) {
        let mut instruction = Instruction::new(format!("LD V{destination:X}, V{source:X}"));
        if destination == ACC {
            instruction.accumulator = Effect::Clobber;
        }
        self.push(instruction);
    }

    fn load_index(&mut self, name: &str) {
        let key = format!("&{name}");
        let mut instruction = Instruction::new(format!("LD I, {name}"));
        instruction.index_register = Effect::Set(key.clone());
        let start = self.push(instruction);

        self.groups.push(Group {
            start,
            end: start,
            value: key,
            accumulator: false,
            disturbs_flag: false,
        });
    }

    /// A register holding this value, loading the accumulator only when the
    /// value is not already sitting in one.
    fn register_argument(&mut self, expr: &Expr) -> Result<u8, CompileError> {
        if let Some(register) = self.register_named(expr) {
            return Ok(register);
        }

        self.eval_accumulator(expr, false)?;
        Ok(ACC)
    }

    /// Puts a value in the accumulator, remembering what was put there so the
    /// same work is not done twice.
    fn eval_accumulator(&mut self, expr: &Expr, comparison: bool) -> Result<(), CompileError> {
        let key = self.value_key(expr);
        let start = self.code.len();

        self.emit_value(expr, comparison)?;

        let end = self.code.len();
        if end == start {
            return Ok(());
        }

        let disturbs_flag = self.code[start..end]
            .iter()
            .any(|instruction| instruction.writes_flag);

        if let Some(key) = key {
            self.code[end - 1].accumulator = Effect::Set(key.clone());
            self.groups.push(Group {
                start,
                end: end - 1,
                value: key,
                accumulator: true,
                disturbs_flag,
            });
        } else {
            self.code[end - 1].accumulator = Effect::Clobber;
        }

        Ok(())
    }

    fn emit_value(&mut self, expr: &Expr, comparison: bool) -> Result<(), CompileError> {
        self.line = expr.line;

        if let Some(value) = self.constant(expr) {
            let byte = self.byte(value)?;
            self.load_immediate(ACC, byte);
            return Ok(());
        }

        match &expr.kind {
            ExprKind::Name(name) => {
                let source = self.lookup_variable(name)?;
                if source != ACC {
                    self.move_register(ACC, source);
                }
                Ok(())
            }
            ExprKind::Register(index) => {
                if *index != ACC {
                    self.move_register(ACC, *index);
                }
                Ok(())
            }
            ExprKind::Delay => {
                let mut instruction = Instruction::new(format!("LD V{ACC:X}, DT"));
                instruction.accumulator = Effect::Clobber;
                self.push(instruction);
                Ok(())
            }
            ExprKind::Index { name, index } => self.emit_index_load(name, index),
            ExprKind::Call { name, arguments } => self.emit_call_value(name, arguments),
            ExprKind::Not(_) => self.error("`!` only makes sense in a condition"),
            ExprKind::Number(_) => unreachable!("numbers are constants"),
            ExprKind::Binary {
                operator,
                left,
                right,
            } => self.emit_binary(*operator, left, right, comparison),
        }
    }

    fn emit_index_load(&mut self, name: &str, index: &Expr) -> Result<(), CompileError> {
        if !self.data.contains_key(name) {
            return self.error(format!("`{name}` is not an array"));
        }

        if let Some(offset) = self.constant(index) {
            let mut instruction = Instruction::new(if offset == 0 {
                format!("LD I, {name}")
            } else {
                format!("LD I, {name} + {offset}")
            });
            instruction.index_register = Effect::Clobber;
            self.push(instruction);
        } else {
            self.load_index(name);
            let register = self.register_argument(index)?;
            let mut instruction = Instruction::new(format!("ADD I, V{register:X}"));
            instruction.index_register = Effect::Clobber;
            self.push(instruction);
        }

        let mut instruction = Instruction::new(format!("LD V{ACC:X}, [I]"));
        instruction.accumulator = Effect::Clobber;
        self.push(instruction);

        Ok(())
    }

    fn emit_call_value(&mut self, name: &str, arguments: &[Expr]) -> Result<(), CompileError> {
        match name {
            "random" => {
                let [mask] = arguments else {
                    return self.error("`random` takes one mask");
                };
                let Some(mask) = self.constant(mask) else {
                    return self.error("the mask given to `random` has to be a constant");
                };
                let mask = self.byte(mask)?;
                let mut instruction = Instruction::new(format!("RND V{ACC:X}, {mask}"));
                instruction.accumulator = Effect::Clobber;
                self.push(instruction);
                Ok(())
            }
            "key" => {
                if !arguments.is_empty() {
                    return self.error("`key` takes no arguments");
                }
                let mut instruction = Instruction::new(format!("LD V{ACC:X}, K"));
                instruction.accumulator = Effect::Clobber;
                self.push(instruction);
                Ok(())
            }
            "pressed" => self.error("`pressed` is a condition, so it belongs in an `if`"),
            other => self.error(format!("`{other}` does not produce a value")),
        }
    }

    fn emit_binary(
        &mut self,
        operator: BinOp,
        left: &Expr,
        right: &Expr,
        comparison: bool,
    ) -> Result<(), CompileError> {
        if operator.is_comparison() {
            return self.error("a comparison only belongs in a condition");
        }

        if matches!(operator, BinOp::Shr | BinOp::Shl) {
            if self.constant(right) != Some(1) {
                return self.error("the machine can only shift by one");
            }
            self.eval_accumulator(left, false)?;
            let mnemonic = if operator == BinOp::Shr { "SHR" } else { "SHL" };
            let mut instruction = Instruction::new(format!("{mnemonic} V{ACC:X}"));
            instruction.writes_flag = true;
            self.push(instruction);
            return Ok(());
        }

        // Taking a variable away from a constant is the one case where the
        // machine's backwards subtract saves an instruction. A comparison
        // always uses the forwards one, because that is the direction the flag
        // is read in.
        if operator == BinOp::Sub
            && !comparison
            && self.constant(left).is_some()
            && self.constant(right).is_none()
        {
            self.eval_accumulator(right, false)?;
            self.load_stage(left)?;
            let mut instruction = Instruction::new(format!("SUBN V{ACC:X}, V{STAGE:X}"));
            instruction.writes_flag = true;
            self.push(instruction);
            return Ok(());
        }

        self.eval_accumulator(left, false)?;

        if operator == BinOp::Add {
            if let Some(value) = self.constant(right) {
                let byte = self.byte(value)?;
                self.emit(format!("ADD V{ACC:X}, {byte}"));
                return Ok(());
            }
        }

        self.load_stage(right)?;

        let mnemonic = match operator {
            BinOp::Add => "ADD",
            BinOp::Sub => "SUB",
            BinOp::And => "AND",
            BinOp::Or => "OR",
            BinOp::Xor => "XOR",
            _ => unreachable!("handled above"),
        };

        let mut instruction = Instruction::new(format!("{mnemonic} V{ACC:X}, V{STAGE:X}"));
        instruction.writes_flag = true;
        self.push(instruction);

        Ok(())
    }

    /// Puts the right hand operand where a two register instruction can reach
    /// it. This slot is never remembered, so it is always written afresh.
    fn load_stage(&mut self, expr: &Expr) -> Result<(), CompileError> {
        if let Some(value) = self.constant(expr) {
            let byte = self.byte(value)?;
            self.emit(format!("LD V{STAGE:X}, {byte}"));
            return Ok(());
        }

        if let ExprKind::Name(name) = &expr.kind {
            let source = self.lookup_variable(name)?;
            self.emit(format!("LD V{STAGE:X}, V{source:X}"));
            return Ok(());
        }

        self.error("this is too much for one expression: work it out into a variable first")
    }

    // -- calls ---------------------------------------------------------------

    fn call(&mut self, name: &str, arguments: &[Expr]) -> Result<(), CompileError> {
        match name {
            "clear" => {
                if !arguments.is_empty() {
                    return self.error("`clear` takes no arguments");
                }
                self.emit("CLS");
                Ok(())
            }
            "draw" => self.draw(arguments),
            "bcd" => {
                let [value] = arguments else {
                    return self.error("`bcd` takes one value");
                };
                let register = self.register_argument(value)?;
                self.emit(format!("LD B, V{register:X}"));
                Ok(())
            }
            "store" | "restore" => {
                let [last] = arguments else {
                    return self.error(format!("`{name}` takes one variable"));
                };
                let register = self.register_argument(last)?;
                if name == "store" {
                    self.emit(format!("LD [I], V{register:X}"));
                } else {
                    let mut instruction = Instruction::new(format!("LD V{register:X}, [I]"));
                    instruction.accumulator = Effect::Clobber;
                    self.push(instruction);
                }
                Ok(())
            }
            other if self.functions.iter().any(|function| function == other) => {
                let mut instruction = Instruction::new(format!("CALL {other}"));
                instruction.flow = Flow::Call;
                instruction.accumulator = Effect::Clobber;
                instruction.index_register = Effect::Clobber;
                self.push(instruction);
                Ok(())
            }
            other => self.error(format!("there is no function called `{other}`")),
        }
    }

    fn draw(&mut self, arguments: &[Expr]) -> Result<(), CompileError> {
        let [x, y, what] = arguments else {
            return self.error("`draw` takes an x, a y and something to draw");
        };

        let x = self.register_argument(x)?;
        let y = self.register_argument(y)?;

        let height = match &what.kind {
            ExprKind::Call { name, arguments } if name == "font" => {
                let [digit] = arguments.as_slice() else {
                    return self.error("`font` takes one digit");
                };
                let register = self.register_argument(digit)?;
                let mut instruction = Instruction::new(format!("LD F, V{register:X}"));
                instruction.index_register = Effect::Clobber;
                self.push(instruction);
                FONT_HEIGHT
            }
            ExprKind::Name(name) if self.data.contains_key(name) => {
                let height = self.data[name];
                if height > 15 {
                    return self.error(format!(
                        "`{name}` is {height} bytes, and a sprite can be at most 15 rows tall"
                    ));
                }
                self.load_index(name);
                u8::try_from(height).unwrap_or(15)
            }
            _ => {
                let Some(height) = self.constant(what) else {
                    return self
                        .error("the third thing to `draw` is a sprite, `font(x)`, or a height");
                };
                if !(1..=15).contains(&height) {
                    return self.error("a sprite is between 1 and 15 rows tall");
                }
                u8::try_from(height).unwrap_or(15)
            }
        };

        let mut instruction = Instruction::new(format!("DRW V{x:X}, V{y:X}, {height}"));
        instruction.writes_flag = true;
        self.push(instruction);

        Ok(())
    }

    // -- working out what is already in place --------------------------------

    /// Follows every path through the program to see what the accumulator and
    /// `I` hold at each instruction, then drops the loads that put something
    /// where it already is.
    ///
    /// Values meet at labels: a label reached from two places only knows what
    /// both places agree on, so nothing is dropped that some other path did
    /// not compute.
    fn analyse(&mut self) {
        let count = self.code.len();
        if count == 0 {
            return;
        }

        let mut targets: HashMap<String, usize> = HashMap::new();
        for (index, instruction) in self.code.iter().enumerate() {
            for label in &instruction.labels {
                targets.insert(label.clone(), index);
            }
        }

        let mut entry = vec![false; count];
        entry[0] = true;
        for (index, instruction) in self.code.iter().enumerate() {
            // A function is only ever reached by `CALL`, from anywhere.
            if instruction
                .labels
                .iter()
                .any(|label| self.functions.contains(label))
            {
                entry[index] = true;
            }
        }

        let mut successors: Vec<Vec<usize>> = vec![Vec::new(); count];
        for (index, instruction) in self.code.iter().enumerate() {
            match &instruction.flow {
                Flow::Next | Flow::Call => {
                    if index + 1 < count {
                        successors[index].push(index + 1);
                    }
                }
                Flow::Skip => {
                    for step in 1..=2 {
                        if index + step < count {
                            successors[index].push(index + step);
                        }
                    }
                }
                Flow::Jump(target) => {
                    if let Some(&target) = targets.get(target) {
                        successors[index].push(target);
                    }
                }
                Flow::Return => {}
            }
        }

        let unvisited = State {
            accumulator: Known::Unvisited,
            index_register: Known::Unvisited,
        };
        let anything = State {
            accumulator: Known::Anything,
            index_register: Known::Anything,
        };

        let mut states = vec![unvisited; count];
        for (index, state) in states.iter_mut().enumerate() {
            if entry[index] {
                *state = anything.clone();
            }
        }

        loop {
            let mut changed = false;

            for index in 0..count {
                let instruction = &self.code[index];
                let leaving = if instruction.flow == Flow::Call {
                    anything.clone()
                } else {
                    State {
                        accumulator: states[index].accumulator.after(&instruction.accumulator),
                        index_register: states[index]
                            .index_register
                            .after(&instruction.index_register),
                    }
                };

                for next in successors[index].clone() {
                    if entry[next] {
                        continue;
                    }

                    let merged = State {
                        accumulator: states[next].accumulator.meet(&leaving.accumulator),
                        index_register: states[next].index_register.meet(&leaving.index_register),
                    };

                    if merged != states[next] {
                        states[next] = merged;
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        let mut deleted = vec![false; count];
        let mut groups = self.groups.clone();
        // Outermost first, so a whole calculation goes before its parts do.
        groups.sort_by_key(|group| (group.start, std::cmp::Reverse(group.end)));

        for group in &groups {
            if (group.start..=group.end).any(|index| deleted[index]) {
                continue;
            }

            let state = &states[group.start];
            let known = if group.accumulator {
                &state.accumulator
            } else {
                &state.index_register
            };

            if *known != Known::Value(group.value.clone()) {
                continue;
            }

            // Dropping a subtraction would leave the flag behind as well.
            if group.disturbs_flag && !self.flag_is_dead_after(group.end) {
                continue;
            }

            for slot in &mut deleted[group.start..=group.end] {
                *slot = true;
            }
        }

        let mut carried: Vec<String> = Vec::new();
        let mut kept = Vec::with_capacity(count);

        for (index, mut instruction) in std::mem::take(&mut self.code).into_iter().enumerate() {
            if deleted[index] {
                carried.append(&mut instruction.labels);
                continue;
            }

            if !carried.is_empty() {
                carried.append(&mut instruction.labels);
                instruction.labels = std::mem::take(&mut carried);
            }

            kept.push(instruction);
        }

        self.code = kept;
    }

    /// Whether the flag is written again before anyone looks at it. Anything
    /// that could be reached from somewhere else counts as a read, because
    /// there is no telling what that other path left behind.
    fn flag_is_dead_after(&self, from: usize) -> bool {
        let mut index = from;

        loop {
            if self.code[index].flow != Flow::Next {
                return false;
            }

            index += 1;
            if index >= self.code.len() {
                return false;
            }

            let instruction = &self.code[index];
            if !instruction.labels.is_empty() || instruction.reads_flag {
                return false;
            }
            if instruction.writes_flag {
                return true;
            }
        }
    }

    // -- output --------------------------------------------------------------

    fn render(&self, data: &[&Data]) -> Result<String, CompileError> {
        let mut out = String::new();

        out.push_str("; Generated by the C8 compiler. Edit the .c8 source instead.\n\n");

        for instruction in &self.code {
            for label in &instruction.labels {
                let _ = writeln!(out, "{label}:");
            }
            let _ = writeln!(out, "    {}", instruction.text);
        }

        for item in data {
            let mut bytes = Vec::with_capacity(item.bytes.len());
            for byte in &item.bytes {
                let Some(value) = self.constant(byte) else {
                    return Err(CompileError {
                        line: item.line,
                        message: format!("every byte of `{}` has to be a constant", item.name),
                    });
                };
                let value = u8::try_from(value).map_err(|_| CompileError {
                    line: item.line,
                    message: format!("{value} does not fit in a byte"),
                })?;
                bytes.push(format!("${value:02X}"));
            }

            let _ = writeln!(out, "\n{}:", item.name);
            for chunk in bytes.chunks(8) {
                let _ = writeln!(out, "    DB {}", chunk.join(", "));
            }
        }

        Ok(out)
    }
}
