//! Display implementations for TPTP AST types.
//!
//! These implementations produce valid TPTP syntax that can be reparsed.

use std::fmt::{self, Display, Formatter};

use super::*;

// =============================================================================
// Common types
// =============================================================================

impl<'a> Display for Name<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Name::Lower(s) => write!(f, "{}", s),
            Name::SingleQuoted(s) => write!(f, "'{}'", s),
            Name::Integer(s) => write!(f, "{}", s),
        }
    }
}

impl<'a> Display for Number<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'a> Display for AtomicWord<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            AtomicWord::Lower(s) => write!(f, "{}", s),
            AtomicWord::SingleQuoted(s) => write!(f, "'{}'", s),
        }
    }
}

impl<'a> Display for DefinedWord<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}

impl<'a> Display for SystemWord<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "$${}", self.0)
    }
}

impl Display for BinaryConnective {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Display for UnaryConnective {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Display for Quantifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Display for InfixEquality {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'a> Display for GeneralTerm<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GeneralTerm::Word(w) => write!(f, "{}", w),
            GeneralTerm::Number(n) => write!(f, "{}", n),
            GeneralTerm::DistinctObject(s) => write!(f, "\"{}\"", s),
            GeneralTerm::Variable(v) => write!(f, "{}", v),
            GeneralTerm::Function(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            GeneralTerm::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            GeneralTerm::ColonPair(left, right) => write!(f, "{}: {}", left, right),
            GeneralTerm::Formula(data) => write!(f, "{}", data),
        }
    }
}

impl<'a> Display for GeneralData<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GeneralData::THF(formula) => write!(f, "$thf({})", formula),
            GeneralData::TFF(formula) => write!(f, "$tff({})", formula),
            GeneralData::FOF(formula) => write!(f, "$fof({})", formula),
            GeneralData::CNF(formula) => write!(f, "$cnf({})", formula),
            GeneralData::FOT(term) => write!(f, "$fot({})", term),
        }
    }
}

impl<'a> Display for Annotations<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.source)?;
        if let Some(info) = &self.useful_info {
            write!(f, ", [")?;
            for (i, item) in info.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", item)?;
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}

// =============================================================================
// FOF types
// =============================================================================

impl<'a> Display for FOFStatement<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FOFStatement::Logical(formula) => write!(f, "{}", formula),
            FOFStatement::Sequent(assumptions, conclusions) => {
                write!(f, "[")?;
                for (i, a) in assumptions.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, "] --> [")?;
                for (i, c) in conclusions.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", c)?;
                }
                write!(f, "]")
            }
        }
    }
}

impl<'a> Display for FOFFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FOFFormula::Atomic(atomic) => write!(f, "{}", atomic),
            FOFFormula::Negation(inner) => write!(f, "~ {}", inner),
            FOFFormula::Quantified {
                quantifier,
                variables,
                formula,
            } => {
                write!(f, "{} [", quantifier)?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]: {}", formula)
            }
            FOFFormula::Binary {
                left,
                connective,
                right,
            } => {
                write!(f, "{} {} {}", left, connective, right)
            }
            FOFFormula::Equality(left, right) => write!(f, "{} = {}", left, right),
            FOFFormula::Inequality(left, right) => write!(f, "{} != {}", left, right),
            FOFFormula::Parens(inner) => write!(f, "({})", inner),
        }
    }
}

impl<'a> Display for FOFAtomicFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FOFAtomicFormula::Plain(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            FOFAtomicFormula::Defined(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            FOFAtomicFormula::System(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            FOFAtomicFormula::True => write!(f, "$true"),
            FOFAtomicFormula::False => write!(f, "$false"),
        }
    }
}

impl<'a> Display for FOFTerm<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FOFTerm::Variable(v) => write!(f, "{}", v),
            FOFTerm::Function(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            FOFTerm::DefinedFunction(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            FOFTerm::SystemFunction(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            FOFTerm::Number(n) => write!(f, "{}", n),
            FOFTerm::DistinctObject(s) => write!(f, "\"{}\"", s),
        }
    }
}

// =============================================================================
// CNF types
// =============================================================================

impl<'a> Display for CNFStatement<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CNFStatement::Logical(formula) => write!(f, "{}", formula),
        }
    }
}

impl<'a> Display for CNFFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CNFFormula::Disjunction(lits) => {
                if lits.is_empty() {
                    write!(f, "$false")
                } else {
                    for (i, lit) in lits.iter().enumerate() {
                        if i > 0 {
                            write!(f, " | ")?;
                        }
                        write!(f, "{}", lit)?;
                    }
                    Ok(())
                }
            }
            CNFFormula::Parens(inner) => write!(f, "({})", inner),
        }
    }
}

impl<'a> Display for CNFLiteral<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CNFLiteral::Positive(atomic) => write!(f, "{}", atomic),
            CNFLiteral::Negative(atomic) => write!(f, "~ {}", atomic),
            CNFLiteral::Equality(left, right) => write!(f, "{} = {}", left, right),
            CNFLiteral::Inequality(left, right) => write!(f, "{} != {}", left, right),
        }
    }
}

impl<'a> Display for CNFAtomicFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CNFAtomicFormula::Plain(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            CNFAtomicFormula::Defined(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            CNFAtomicFormula::System(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            CNFAtomicFormula::True => write!(f, "$true"),
            CNFAtomicFormula::False => write!(f, "$false"),
        }
    }
}

// =============================================================================
// TFF types
// =============================================================================

impl<'a> Display for TFFStatement<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TFFStatement::Logical(formula) => write!(f, "{}", formula),
            TFFStatement::Typing(typing) => write!(f, "{}", typing),
            TFFStatement::Sequent(assumptions, conclusions) => {
                write!(f, "[")?;
                for (i, a) in assumptions.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, "] --> [")?;
                for (i, c) in conclusions.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", c)?;
                }
                write!(f, "]")
            }
            TFFStatement::Logic(spec) => write!(f, "{}", spec),
        }
    }
}

impl<'a> Display for TFFTyping<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.symbol, self.typ)
    }
}

impl<'a> Display for TypedSymbol<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TypedSymbol::Atom(a) => write!(f, "{}", a),
            TypedSymbol::Defined(d) => write!(f, "{}", d),
        }
    }
}

impl<'a> Display for TFFFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TFFFormula::Atomic(atomic) => write!(f, "{}", atomic),
            TFFFormula::Negation(inner) => write!(f, "~ {}", inner),
            TFFFormula::Quantified {
                quantifier,
                variables,
                formula,
            } => {
                write!(f, "{} [", quantifier)?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]: {}", formula)
            }
            TFFFormula::TypeQuantified {
                quantifier,
                type_variables,
                formula,
            } => {
                write!(f, "{} [", quantifier)?;
                for (i, v) in type_variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]: {}", formula)
            }
            TFFFormula::Binary {
                left,
                connective,
                right,
            } => {
                write!(f, "{} {} {}", left, connective, right)
            }
            TFFFormula::Equality(left, right) => write!(f, "{} = {}", left, right),
            TFFFormula::Inequality(left, right) => write!(f, "{} != {}", left, right),
            TFFFormula::Parens(inner) => write!(f, "({})", inner),
            TFFFormula::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                write!(f, "$ite({}, {}, {})", condition, then_branch, else_branch)
            }
            TFFFormula::Let { definitions, body } => {
                write!(f, "$let(")?;
                // For single definition, output as: symbol: type, symbol := value, body
                if definitions.len() == 1 {
                    let def = &definitions[0];
                    // Type declaration part
                    write!(f, "{}", def.symbol)?;
                    if !def.type_args.is_empty() || !def.params.is_empty() {
                        write!(f, "(")?;
                        let mut first = true;
                        for ta in &def.type_args {
                            if !first {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", ta)?;
                            first = false;
                        }
                        for p in &def.params {
                            if !first {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", p)?;
                            first = false;
                        }
                        write!(f, ")")?;
                    }
                    if let Some(typ) = &def.typ {
                        write!(f, ": {}", typ)?;
                    }
                    // Assignment part
                    write!(f, ", ")?;
                    write!(f, "{}", def.symbol)?;
                    if !def.params.is_empty() {
                        write!(f, "(")?;
                        for (i, p) in def.params.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", p.name)?;
                        }
                        write!(f, ")")?;
                    }
                    write!(f, " := {}", def.definition)?;
                } else {
                    // For tuple let: $let([types], [vars] := value, body) or $let([types], [assignments], body)
                    // Check if all definitions have the same value (tuple unpacking case)
                    let all_same_value = definitions.len() > 1
                        && definitions.windows(2).all(|w| {
                            format!("{}", w[0].definition) == format!("{}", w[1].definition)
                        });

                    // Type declarations
                    write!(f, "[")?;
                    for (i, def) in definitions.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", def.symbol)?;
                        if !def.type_args.is_empty() || !def.params.is_empty() {
                            write!(f, "(")?;
                            let mut first = true;
                            for ta in &def.type_args {
                                if !first {
                                    write!(f, ", ")?;
                                }
                                write!(f, "{}", ta)?;
                                first = false;
                            }
                            for p in &def.params {
                                if !first {
                                    write!(f, ", ")?;
                                }
                                write!(f, "{}", p)?;
                                first = false;
                            }
                            write!(f, ")")?;
                        }
                        if let Some(typ) = &def.typ {
                            write!(f, ": {}", typ)?;
                        }
                    }
                    write!(f, "], ")?;

                    if all_same_value {
                        // Tuple unpacking: [var1, var2, ...] := shared_value
                        write!(f, "[")?;
                        for (i, def) in definitions.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", def.symbol)?;
                        }
                        write!(f, "] := {}", definitions[0].definition)?;
                    } else {
                        // Individual assignments: [var1 := val1, var2 := val2]
                        write!(f, "[")?;
                        for (i, def) in definitions.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", def.symbol)?;
                            if !def.params.is_empty() {
                                write!(f, "(")?;
                                for (j, p) in def.params.iter().enumerate() {
                                    if j > 0 {
                                        write!(f, ", ")?;
                                    }
                                    write!(f, "{}", p.name)?;
                                }
                                write!(f, ")")?;
                            }
                            write!(f, " := {}", def.definition)?;
                        }
                        write!(f, "]")?;
                    }
                }
                write!(f, ", {})", body)
            }
            TFFFormula::NonClassical { operator, formula } => {
                // Short-form operators use direct application without @
                match operator {
                    NonClassicalOperator::ShortBox(_) | NonClassicalOperator::ShortDiamond(_) => {
                        write!(f, "{}({})", operator, formula)
                    }
                    _ => {
                        // Check if the formula needs parentheses after @
                        // Binary connectives have lower precedence than @, so they need parens
                        let needs_parens = matches!(formula.as_ref(), TFFFormula::Binary { .. });
                        if needs_parens {
                            write!(f, "{} @ ({})", operator, formula)
                        } else {
                            write!(f, "{} @ {}", operator, formula)
                        }
                    }
                }
            }
        }
    }
}

impl<'a> Display for TFFVariable<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(typ) = &self.typ {
            write!(f, ": {}", typ)?;
        }
        Ok(())
    }
}

impl Display for TypeQuantifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'a> Display for TFFAtomicFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TFFAtomicFormula::Plain(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TFFAtomicFormula::Defined(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TFFAtomicFormula::System(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TFFAtomicFormula::True => write!(f, "$true"),
            TFFAtomicFormula::False => write!(f, "$false"),
            TFFAtomicFormula::Variable(v) => write!(f, "{}", v),
        }
    }
}

impl<'a> Display for TFFTerm<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TFFTerm::Variable(v) => write!(f, "{}", v),
            TFFTerm::Function(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TFFTerm::DefinedFunction(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TFFTerm::SystemFunction(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TFFTerm::Number(n) => write!(f, "{}", n),
            TFFTerm::DistinctObject(s) => write!(f, "\"{}\"", s),
            TFFTerm::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                write!(f, "$ite({}, {}, {})", condition, then_branch, else_branch)
            }
            TFFTerm::Let { definitions, body } => {
                write!(f, "$let(")?;
                if definitions.len() == 1 {
                    let def = &definitions[0];
                    // Type declaration
                    write!(f, "{}", def.symbol)?;
                    if !def.type_args.is_empty() || !def.params.is_empty() {
                        write!(f, "(")?;
                        let mut first = true;
                        for ta in &def.type_args {
                            if !first {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", ta)?;
                            first = false;
                        }
                        for p in &def.params {
                            if !first {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", p)?;
                            first = false;
                        }
                        write!(f, ")")?;
                    }
                    if let Some(typ) = &def.typ {
                        write!(f, ": {}", typ)?;
                    }
                    // Assignment
                    write!(f, ", {} := {}", def.symbol, def.definition)?;
                } else {
                    // Check if all definitions have the same value (tuple unpacking case)
                    let all_same_value = definitions.len() > 1
                        && definitions.windows(2).all(|w| {
                            format!("{}", w[0].definition) == format!("{}", w[1].definition)
                        });

                    // Type declarations
                    write!(f, "[")?;
                    for (i, def) in definitions.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", def.symbol)?;
                        if !def.type_args.is_empty() || !def.params.is_empty() {
                            write!(f, "(")?;
                            let mut first = true;
                            for ta in &def.type_args {
                                if !first {
                                    write!(f, ", ")?;
                                }
                                write!(f, "{}", ta)?;
                                first = false;
                            }
                            for p in &def.params {
                                if !first {
                                    write!(f, ", ")?;
                                }
                                write!(f, "{}", p)?;
                                first = false;
                            }
                            write!(f, ")")?;
                        }
                        if let Some(typ) = &def.typ {
                            write!(f, ": {}", typ)?;
                        }
                    }
                    write!(f, "], ")?;

                    if all_same_value {
                        // Tuple unpacking: [var1, var2, ...] := shared_value
                        write!(f, "[")?;
                        for (i, def) in definitions.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", def.symbol)?;
                        }
                        write!(f, "] := {}", definitions[0].definition)?;
                    } else {
                        // Individual assignments: [var1 := val1, var2 := val2]
                        write!(f, "[")?;
                        for (i, def) in definitions.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", def.symbol)?;
                            if !def.params.is_empty() {
                                write!(f, "(")?;
                                for (j, p) in def.params.iter().enumerate() {
                                    if j > 0 {
                                        write!(f, ", ")?;
                                    }
                                    write!(f, "{}", p.name)?;
                                }
                                write!(f, ")")?;
                            }
                            write!(f, " := {}", def.definition)?;
                        }
                        write!(f, "]")?;
                    }
                }
                write!(f, ", {})", body)
            }
            TFFTerm::Tuple(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            TFFTerm::FormulaAsTerm(formula) => {
                write!(f, "({})", formula)
            }
            TFFTerm::Parens(term) => {
                write!(f, "({})", term)
            }
        }
    }
}

impl<'a> Display for TFFLetDef<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol)?;
        if !self.type_args.is_empty() || !self.params.is_empty() {
            write!(f, "(")?;
            let mut first = true;
            for ta in &self.type_args {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", ta)?;
                first = false;
            }
            for p in &self.params {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", p)?;
                first = false;
            }
            write!(f, ")")?;
        }
        if let Some(typ) = &self.typ {
            write!(f, ": {}", typ)?;
        }
        write!(f, " := {}", self.definition)
    }
}

impl<'a> Display for TFFLetBody<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TFFLetBody::Formula(formula) => write!(f, "{}", formula),
            TFFLetBody::Term(term) => write!(f, "{}", term),
        }
    }
}

impl<'a> Display for TFFType<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TFFType::Atomic(name) => write!(f, "{}", name),
            TFFType::Defined(dt) => write!(f, "{}", dt),
            TFFType::Variable(v) => write!(f, "{}", v),
            TFFType::Function { args, result } => {
                if args.len() == 1 {
                    write!(f, "{} > {}", args[0], result)
                } else {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, " * ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ") > {}", result)
                }
            }
            TFFType::Application(base, args) => {
                write!(f, "{}", base)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TFFType::Tuple(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            TFFType::Quantified { variables, typ } => {
                write!(f, "!> [")?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]: {}", typ)
            }
            TFFType::Parens(inner) => write!(f, "({})", inner),
        }
    }
}

impl Display for DefinedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// TCF types
// =============================================================================

impl<'a> Display for TCFStatement<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TCFStatement::Logical(formula) => write!(f, "{}", formula),
            TCFStatement::Typing(typing) => write!(f, "{}", typing),
        }
    }
}

impl<'a> Display for TCFTyping<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.symbol, self.typ)
    }
}

impl<'a> Display for TCFFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TCFFormula::Quantified { variables, clause } => {
                write!(f, "! [")?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]: {}", clause)
            }
            TCFFormula::Clause(clause) => write!(f, "{}", clause),
        }
    }
}

impl<'a> Display for TCFClause<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TCFClause::Disjunction(lits) => {
                if lits.is_empty() {
                    write!(f, "$false")
                } else {
                    for (i, lit) in lits.iter().enumerate() {
                        if i > 0 {
                            write!(f, " | ")?;
                        }
                        write!(f, "{}", lit)?;
                    }
                    Ok(())
                }
            }
            TCFClause::Parens(inner) => write!(f, "({})", inner),
        }
    }
}

impl<'a> Display for TCFLiteral<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TCFLiteral::Positive(atomic) => write!(f, "{}", atomic),
            TCFLiteral::Negative(atomic) => write!(f, "~ {}", atomic),
            TCFLiteral::Equality(left, right) => write!(f, "{} = {}", left, right),
            TCFLiteral::Inequality(left, right) => write!(f, "{} != {}", left, right),
            TCFLiteral::Parens(inner) => write!(f, "({})", inner),
        }
    }
}

impl<'a> Display for TCFAtomicFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TCFAtomicFormula::Plain(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TCFAtomicFormula::Defined(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TCFAtomicFormula::System(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            TCFAtomicFormula::True => write!(f, "$true"),
            TCFAtomicFormula::False => write!(f, "$false"),
        }
    }
}

// =============================================================================
// THF types
// =============================================================================

impl<'a> Display for THFStatement<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            THFStatement::Logical(formula) => write!(f, "{}", formula),
            THFStatement::Typing(typing) => write!(f, "{}", typing),
            THFStatement::Subtype(sub, sup) => write!(f, "{} << {}", sub, sup),
            THFStatement::Sequent(assumptions, conclusions) => {
                write!(f, "[")?;
                for (i, a) in assumptions.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, "] --> [")?;
                for (i, c) in conclusions.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", c)?;
                }
                write!(f, "]")
            }
            THFStatement::Logic(spec) => write!(f, "{}", spec),
        }
    }
}

impl<'a> Display for THFTyping<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.symbol, self.typ)
    }
}

impl<'a> Display for THFTypedSymbol<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            THFTypedSymbol::Atom(a) => write!(f, "{}", a),
            THFTypedSymbol::Defined(d) => write!(f, "{}", d),
            THFTypedSymbol::System(s) => write!(f, "{}", s),
        }
    }
}

impl<'a> Display for THFFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            THFFormula::Atomic(atomic) => write!(f, "{}", atomic),
            THFFormula::Variable(v) => write!(f, "{}", v),
            THFFormula::Negation(inner) => write!(f, "~ {}", inner),
            THFFormula::Quantified {
                quantifier,
                variables,
                formula,
            } => {
                write!(f, "{} [", quantifier)?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]: {}", formula)
            }
            THFFormula::Binary {
                left,
                connective,
                right,
            } => {
                write!(f, "{} {} {}", left, connective, right)
            }
            THFFormula::Application(func, arg) => {
                // TypeAsFormula with mapping or arrow needs parens as argument
                let needs_parens = matches!(arg.as_ref(),
                    THFFormula::TypeAsFormula(t) if matches!(t,
                        THFType::Mapping(_, _) | THFType::Arrow(_, _) | THFType::Product(_, _) | THFType::Sum(_, _)
                    )
                );
                if needs_parens {
                    write!(f, "{} @ ({})", func, arg)
                } else {
                    write!(f, "{} @ {}", func, arg)
                }
            }
            THFFormula::Lambda { variables, body } => {
                write!(f, "^ [")?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]: {}", body)
            }
            THFFormula::Equality(left, right) => write!(f, "{} = {}", left, right),
            THFFormula::Inequality(left, right) => write!(f, "{} != {}", left, right),
            THFFormula::Parens(inner) => write!(f, "({})", inner),
            THFFormula::Typed(formula, typ) => write!(f, "{}: {}", formula, typ),
            THFFormula::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                write!(f, "$ite({}, {}, {})", condition, then_branch, else_branch)
            }
            THFFormula::Let { definitions, body } => {
                write!(f, "$let(")?;
                if definitions.len() == 1 {
                    write!(f, "{}", definitions[0])?;
                } else {
                    write!(f, "[")?;
                    for (i, def) in definitions.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", def)?;
                    }
                    write!(f, "]")?;
                }
                write!(f, ", {})", body)
            }
            THFFormula::Tuple(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            THFFormula::Number(n) => write!(f, "{}", n),
            THFFormula::DistinctObject(s) => write!(f, "\"{}\"", s),
            THFFormula::NonClassical { operator, formula } => {
                write!(f, "{}", operator)?;
                if let Some(formula) = formula {
                    // Short-form operators use direct application without @
                    match operator {
                        NonClassicalOperator::ShortBox(_)
                        | NonClassicalOperator::ShortDiamond(_) => {
                            write!(f, "({})", formula)?;
                        }
                        _ => {
                            // Check if the formula needs parentheses after @
                            // Binary connectives have lower precedence than @, so they need parens
                            let needs_parens =
                                matches!(formula.as_ref(), THFFormula::Binary { .. });
                            if needs_parens {
                                write!(f, " @ ({})", formula)?;
                            } else {
                                write!(f, " @ {}", formula)?;
                            }
                        }
                    }
                }
                Ok(())
            }
            THFFormula::ConnectiveTerm(conn) => write!(f, "({})", conn),
            THFFormula::UnaryConnectiveTerm => write!(f, "(~)"),
            THFFormula::QuantifierTerm(q) => match q {
                THFQuantifier::Forall => write!(f, "(!!)"),
                THFQuantifier::Exists => write!(f, "(??)"),
                THFQuantifier::Choice => write!(f, "(@@+)"),
                THFQuantifier::Description => write!(f, "(@@-)"),
                _ => write!(f, "({})", q),
            },
            THFFormula::EqualityTerm => write!(f, "(@=)"),
            THFFormula::TypeAsFormula(typ) => write!(f, "{}", typ),
        }
    }
}

impl<'a> Display for THFVariable<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(typ) = &self.typ {
            write!(f, ": {}", typ)?;
        }
        Ok(())
    }
}

impl Display for THFQuantifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Display for THFBinaryConnective {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'a> Display for THFAtomicFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            THFAtomicFormula::Plain(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            THFAtomicFormula::Defined(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            THFAtomicFormula::System(name, args) => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            THFAtomicFormula::True => write!(f, "$true"),
            THFAtomicFormula::False => write!(f, "$false"),
        }
    }
}

impl<'a> Display for THFLetDef<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol)?;
        if !self.type_args.is_empty() || !self.params.is_empty() {
            write!(f, "(")?;
            let mut first = true;
            for ta in &self.type_args {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", ta)?;
                first = false;
            }
            for p in &self.params {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{}", p)?;
                first = false;
            }
            write!(f, ")")?;
        }
        write!(f, " := {}", self.definition)
    }
}

impl<'a> Display for THFType<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            THFType::Atomic(name) => write!(f, "{}", name),
            THFType::Defined(dt) => write!(f, "{}", dt),
            THFType::Variable(v) => write!(f, "{}", v),
            THFType::Arrow(from, to) => write!(f, "{} > {}", from, to),
            THFType::Product(left, right) => write!(f, "{} * {}", left, right),
            THFType::Sum(left, right) => write!(f, "{} + {}", left, right),
            THFType::Application(base, args) => {
                write!(f, "{}", base)?;
                for arg in args {
                    write!(f, " @ {}", arg)?;
                }
                Ok(())
            }
            THFType::Tuple(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            THFType::Quantified {
                quantifier,
                variables,
                typ,
            } => {
                write!(f, "{} [", quantifier)?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: $tType", v)?;
                }
                write!(f, "]: {}", typ)
            }
            THFType::DependentQuantified {
                quantifier,
                variables,
                typ,
            } => {
                write!(f, "{} [", quantifier)?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v.name)?;
                    if let Some(t) = &v.typ {
                        write!(f, ": {}", t)?;
                    }
                }
                write!(f, "]: {}", typ)
            }
            THFType::Parens(inner) => write!(f, "({})", inner),
            THFType::Mapping(args, result) => {
                // Single argument doesn't need parens
                if args.len() == 1 {
                    write!(f, "{} > {}", args[0], result)
                } else {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, " * ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ") > {}", result)
                }
            }
        }
    }
}

impl Display for THFDefinedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'a> Display for NonClassicalOperator<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NonClassicalOperator::Box => write!(f, "{{$box}}"),
            NonClassicalOperator::Diamond => write!(f, "{{$dia}}"),
            NonClassicalOperator::Always => write!(f, "{{$always}}"),
            NonClassicalOperator::Eventually => write!(f, "{{$eventually}}"),
            NonClassicalOperator::Knows => write!(f, "{{$knows}}"),
            NonClassicalOperator::Believes => write!(f, "{{$believes}}"),
            NonClassicalOperator::ShortBox(None) => write!(f, "[.]"),
            NonClassicalOperator::ShortBox(Some(idx)) => write!(f, "[#{}]", idx),
            NonClassicalOperator::ShortDiamond(None) => write!(f, "<.>"),
            NonClassicalOperator::ShortDiamond(Some(idx)) => write!(f, "<#{}>", idx),
            NonClassicalOperator::Custom { name, index } => {
                write!(f, "{{${}", name)?;
                if let Some(idx) = index {
                    write!(f, "(#{})", idx)?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl<'a> Display for LogicSpecification<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} == [", self.logic_family)?;
        for (i, prop) in self.properties.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", prop)?;
        }
        write!(f, "]")
    }
}

impl<'a> Display for LogicProperty<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            LogicProperty::KeyValue { key, value } => write!(f, "{} == {}", key, value),
            LogicProperty::KeyList { key, values } => {
                write!(f, "{} == [", key)?;
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
        }
    }
}

impl<'a> Display for LogicValue<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            LogicValue::Atom(s) => write!(f, "{}", s),
            LogicValue::String(s) => write!(f, "'{}'", s),
            LogicValue::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            LogicValue::Property { name, value } => write!(f, "{} == {}", name, value),
        }
    }
}

// =============================================================================
// Top-level types
// =============================================================================

impl Display for FormulaRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'a> Display for Include<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "include('{}'", self.file_name)?;
        if let Some(selection) = &self.selection {
            write!(f, ", [")?;
            for (i, s) in selection.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", s)?;
            }
            write!(f, "]")?;
        }
        write!(f, ").")
    }
}

impl<'a> Display for AnnotatedFormula<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            AnnotatedFormula::THF(ann) => write!(f, "{}", ann),
            AnnotatedFormula::TFF(ann) => write!(f, "{}", ann),
            AnnotatedFormula::FOF(ann) => write!(f, "{}", ann),
            AnnotatedFormula::TCF(ann) => write!(f, "{}", ann),
            AnnotatedFormula::CNF(ann) => write!(f, "{}", ann),
            AnnotatedFormula::TPI(ann) => write!(f, "{}", ann),
        }
    }
}

impl<'a> Display for THFAnnotated<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "thf({}, {}, {}", self.name, self.role, self.formula)?;
        if let Some(ann) = &self.annotations {
            write!(f, ", {}", ann)?;
        }
        write!(f, ").")
    }
}

impl<'a> Display for TFFAnnotated<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "tff({}, {}, {}", self.name, self.role, self.formula)?;
        if let Some(ann) = &self.annotations {
            write!(f, ", {}", ann)?;
        }
        write!(f, ").")
    }
}

impl<'a> Display for FOFAnnotated<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "fof({}, {}, {}", self.name, self.role, self.formula)?;
        if let Some(ann) = &self.annotations {
            write!(f, ", {}", ann)?;
        }
        write!(f, ").")
    }
}

impl<'a> Display for TCFAnnotated<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "tcf({}, {}, {}", self.name, self.role, self.formula)?;
        if let Some(ann) = &self.annotations {
            write!(f, ", {}", ann)?;
        }
        write!(f, ").")
    }
}

impl<'a> Display for CNFAnnotated<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "cnf({}, {}, {}", self.name, self.role, self.formula)?;
        if let Some(ann) = &self.annotations {
            write!(f, ", {}", ann)?;
        }
        write!(f, ").")
    }
}

impl<'a> Display for TPIAnnotated<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "tpi({}, {}, {}", self.name, self.role, self.formula)?;
        if let Some(ann) = &self.annotations {
            write!(f, ", {}", ann)?;
        }
        write!(f, ").")
    }
}
