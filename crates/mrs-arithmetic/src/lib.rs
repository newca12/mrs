//! Exact TPTP integer/rational evaluation, without floating-point arithmetic.
//!
//! This is an evaluator, not a decision procedure for quantified arithmetic.
//! Unsupported expressions, unspecified division by zero, and exhausted resource
//! budgets return errors: callers must not reinterpret them as arbitrary symbols
//! when claiming satisfiability. Integer and rational *types* remain distinct,
//! including integral rationals such as `2/1`.

mod evaluate;
mod interpreted;
#[cfg(test)]
mod tests;

pub use interpreted::{
    requires_cnf_interpretation, requires_interpretation, requires_tcf_interpretation,
};

use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use std::cmp::Ordering;
use std::fmt;

/// A value in one of TPTP's disjoint numeric sorts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Integer(BigInt),
    Rational(BigRational),
}

impl Value {
    fn ratio(&self) -> BigRational {
        match self {
            Self::Integer(n) => BigRational::from_integer(n.clone()),
            Self::Rational(r) => BigRational::new(r.numer().clone(), r.denom().clone()),
        }
    }

    fn same_sort(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Integer(_), Self::Integer(_)) | (Self::Rational(_), Self::Rational(_))
        )
    }

    fn preserve_sort(&self, value: BigRational) -> Self {
        match self {
            Self::Integer(_) => Self::Integer(value.to_integer()),
            Self::Rational(_) => Self::Rational(value),
        }
    }

    fn bits(&self) -> u64 {
        match self {
            Self::Integer(n) => n.bits(),
            Self::Rational(r) => r.numer().bits().max(r.denom().bits()),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(n) => n.fmt(f),
            // Never print an integral rational as an integer: that changes its sort.
            Self::Rational(r) => write!(f, "{}/{}", r.numer(), r.denom()),
        }
    }
}

/// Evaluation is bounded by resources, never by fixed-width numeric overflow.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_bits: u64,
    pub max_work: u64,
    pub deadline: Option<std::time::Instant>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_nodes: 100_000,
            max_depth: 256,
            max_bits: 65_536,
            max_work: 10_000_000,
            deadline: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unsupported,
    InvalidNumber,
    TypeMismatch,
    WrongArity,
    Unspecified,
    ResourceLimit,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unsupported => "unsupported or non-ground arithmetic expression",
            Self::InvalidNumber => "invalid numeric literal",
            Self::TypeMismatch => "incompatible arithmetic sorts",
            Self::WrongArity => "incorrect arithmetic arity",
            Self::Unspecified => "unspecified arithmetic operation (division by zero)",
            Self::ResourceLimit => "arithmetic evaluation resource limit",
        })
    }
}

impl std::error::Error for Error {}

/// One evaluation budget. Reuse it across expressions to bound aggregate work.
pub struct Evaluator {
    limits: Limits,
    nodes: usize,
    work: u64,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}

impl Evaluator {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            nodes: 0,
            work: 0,
        }
    }

    pub fn work_used(&self) -> u64 {
        self.work
    }

    fn enter(&mut self, depth: usize) -> Result<(), Error> {
        self.nodes = self.nodes.checked_add(1).ok_or(Error::ResourceLimit)?;
        if self.nodes > self.limits.max_nodes || depth > self.limits.max_depth {
            return Err(Error::ResourceLimit);
        }
        self.charge(1)
    }

    fn charge(&mut self, work: u64) -> Result<(), Error> {
        if self
            .limits
            .deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            return Err(Error::ResourceLimit);
        }
        self.work = self.work.checked_add(work).ok_or(Error::ResourceLimit)?;
        if self.work > self.limits.max_work {
            Err(Error::ResourceLimit)
        } else {
            Ok(())
        }
    }

    fn check(&self, value: Value) -> Result<Value, Error> {
        if value.bits() > self.limits.max_bits {
            Err(Error::ResourceLimit)
        } else {
            Ok(value)
        }
    }

    fn integer(&mut self, text: &str) -> Result<BigInt, Error> {
        // Check before allocating an arbitrary-precision value. The exact bit
        // check below handles the small overestimate permitted by this bound.
        if text.len() as u64 > self.limits.max_bits / 3 + 2 {
            return Err(Error::ResourceLimit);
        }
        self.charge(text.len() as u64)?;
        let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::InvalidNumber);
        }
        let value = text.parse::<BigInt>().map_err(|_| Error::InvalidNumber)?;
        if value.bits() > self.limits.max_bits {
            Err(Error::ResourceLimit)
        } else {
            Ok(value)
        }
    }

    pub fn number(&mut self, number: &mrs_tptp::Number<'_>) -> Result<Value, Error> {
        use mrs_tptp::Number;
        match number {
            Number::Integer(text) => self.integer(text).map(Value::Integer),
            Number::Rational(text) => {
                let (n, d) = text.split_once('/').ok_or(Error::InvalidNumber)?;
                let n = self.integer(n)?;
                let d = self.integer(d)?;
                if d <= BigInt::zero() {
                    return Err(Error::InvalidNumber);
                }
                self.charge((n.bits() / 64 + 1).saturating_mul(d.bits() / 64 + 1))?;
                self.check(Value::Rational(BigRational::new(n, d)))
            }
            Number::Real(_) => Err(Error::Unsupported),
        }
    }

    fn operands(&mut self, args: &[Value], arity: usize) -> Result<(), Error> {
        if args.len() != arity {
            return Err(Error::WrongArity);
        }
        let mut cost = 1u64;
        let mut normalization = 0u64;
        for value in args {
            if value.bits() > self.limits.max_bits {
                return Err(Error::ResourceLimit);
            }
            if let Value::Rational(r) = value
                && !r.denom().is_positive()
            {
                return Err(Error::InvalidNumber);
            }
            let limbs = value.bits() / 64 + 1;
            cost = cost.saturating_mul(limbs);
            if matches!(value, Value::Rational(_)) {
                normalization = normalization.saturating_add(limbs.saturating_mul(limbs));
            }
        }
        self.charge(cost.saturating_add(normalization))
    }

    /// Evaluate an arithmetic functor, using its name without the initial `$`.
    pub fn function(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        match name {
            "uminus" | "abs" | "floor" | "ceiling" | "truncate" | "round" | "to_int" | "to_rat" => {
                self.operands(args, 1)?;
                let r = args[0].ratio();
                let value = match name {
                    "uminus" => args[0].preserve_sort(-r),
                    "abs" => args[0].preserve_sort(r.abs()),
                    // TPTP v10 ARI525_1 requires floor conversion for negative
                    // nonintegral operands, not truncation toward zero.
                    "to_int" => Value::Integer(round_integral(&r, "floor")),
                    "to_rat" => Value::Rational(r),
                    rounding => args[0]
                        .preserve_sort(BigRational::from_integer(round_integral(&r, rounding))),
                };
                self.check(value)
            }
            "sum" | "difference" | "product" | "quotient" | "quotient_e" | "quotient_t"
            | "quotient_f" | "remainder_e" | "remainder_t" | "remainder_f" => {
                self.operands(args, 2)?;
                if !args[0].same_sort(&args[1]) {
                    return Err(Error::TypeMismatch);
                }
                let a = args[0].ratio();
                let b = args[1].ratio();
                let value = match name {
                    "sum" => args[0].preserve_sort(a + b),
                    "difference" => args[0].preserve_sort(a - b),
                    "product" => args[0].preserve_sort(a * b),
                    operation => {
                        if b.is_zero() {
                            return Err(Error::Unspecified);
                        }
                        let quotient = &a / &b;
                        if operation == "quotient" {
                            // Integer division with $quotient has rational result.
                            Value::Rational(quotient)
                        } else {
                            let rounding = match operation.as_bytes().last() {
                                Some(b'e') if b.is_negative() => "ceiling",
                                Some(b'e' | b'f') => "floor",
                                _ => "truncate",
                            };
                            let integral =
                                BigRational::from_integer(round_integral(&quotient, rounding));
                            let result = if operation.starts_with("remainder") {
                                a - integral * b
                            } else {
                                integral
                            };
                            args[0].preserve_sort(result)
                        }
                    }
                };
                self.check(value)
            }
            _ => Err(Error::Unsupported),
        }
    }

    /// Evaluate a predicate (or numeric `=`), without a leading `$`.
    pub fn predicate(&mut self, name: &str, args: &[Value]) -> Result<bool, Error> {
        match name {
            "is_int" | "is_rat" => {
                self.operands(args, 1)?;
                Ok(name == "is_rat" || args[0].ratio().is_integer())
            }
            "less" | "lesseq" | "greater" | "greatereq" | "=" => {
                self.operands(args, 2)?;
                if !args[0].same_sort(&args[1]) {
                    return Err(Error::TypeMismatch);
                }
                let order = args[0].ratio().cmp(&args[1].ratio());
                Ok(match name {
                    "less" => order == Ordering::Less,
                    "lesseq" => order != Ordering::Greater,
                    "greater" => order == Ordering::Greater,
                    "greatereq" => order != Ordering::Less,
                    _ => order == Ordering::Equal,
                })
            }
            "distinct" => {
                if args.is_empty() {
                    return Err(Error::WrongArity);
                }
                let mut seen = std::collections::HashSet::new();
                for value in args {
                    self.operands(std::slice::from_ref(value), 1)?;
                    if !args[0].same_sort(value) {
                        return Err(Error::TypeMismatch);
                    }
                }
                Ok(args.iter().all(|value| seen.insert(value.ratio())))
            }
            _ => Err(Error::Unsupported),
        }
    }
}

fn round_integral(value: &BigRational, mode: &str) -> BigInt {
    let n = value.numer();
    let d = value.denom();
    match mode {
        "floor" => n.div_floor(d),
        "ceiling" => n.div_ceil(d),
        "truncate" => n / d,
        "round" => {
            let (floor, remainder) = n.div_mod_floor(d);
            match (&remainder * 2u8).cmp(d) {
                Ordering::Less => floor,
                Ordering::Equal if floor.is_even() => floor,
                _ => floor + BigInt::one(),
            }
        }
        _ => unreachable!("rounding modes are internal constants"),
    }
}
