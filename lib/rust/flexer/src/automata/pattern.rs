//! Simple API for constructing regex patterns that are used in parser implementation.

use crate::automata::symbol::Symbol;

use core::iter;
use itertools::Itertools;
use std::ops::BitOr;
use std::ops::RangeInclusive;
use std::ops::Shr;

use Pattern::*;



// =============
// == Pattern ==
// =============

/// A representation of a simple regular pattern.
#[derive(Clone,Debug,PartialEq)]
pub enum Pattern {
    /// The pattern that triggers on any symbol from the given range.
    Range(RangeInclusive<Symbol>),
    /// The pattern that triggers on any given pattern from a sequence.
    Or(Vec<Pattern>),
    /// The pattern that triggers when a sequence of patterns is encountered.
    Seq(Vec<Pattern>),
    /// The pattern that triggers on 0..N repetitions of given pattern.
    Many(Box<Pattern>),
    /// The pattern that always triggers.
    Always,
    /// The pattern that never triggers.
    Never,
}

impl Pattern {

    /// A pattern that never triggers.
    pub fn never() -> Self {
        Pattern::Never
    }

    /// A pattern that always triggers
    pub fn always() -> Self {
        Pattern::Always
    }

    /// A pattern that triggers on any character.
    pub fn any() -> Self {
        Pattern::symbols(Symbol::from(0)..=Symbol::from(u32::max_value()))
    }

    /// A pattern that triggers on 0..N repetitions of the pattern described by `self`.
    pub fn many(&self) -> Self {
        Many(Box::new(self.clone()))
    }

    /// A pattern that triggers on 1..N repetitions of the pattern described by `self`.
    pub fn many1(&self) -> Self {
        self.clone() >> self.many()
    }

    /// A pattern that triggers on 0..=1 repetitions of the pattern described by `self`.
    pub fn opt(&self) -> Self {
        self.clone() | Self::always()
    }

    /// A pattern that triggers on the given character.
    pub fn char(character:char) -> Self {
        Self::symbol(Symbol::from(character))
    }

    /// A pattern that triggers on the given symbol.
    pub fn symbol(symbol:Symbol) -> Self {
        Pattern::symbols(symbol..=symbol)
    }

    /// A pattern that triggers on any of the provided `symbols`.
    pub fn symbols(symbols:RangeInclusive<Symbol>) -> Self {
        Pattern::Range(symbols)
    }

    /// A pattern that triggers at the end of the file.
    pub fn eof() -> Self {
        Self::symbol(Symbol::EOF_CODE)
    }

    /// A pattern that triggers on any character in the provided `range`.
    pub fn range(range:RangeInclusive<char>) -> Self {
        Pattern::symbols(Symbol::from(*range.start())..=Symbol::from(*range.end()))
    }

    /// Pattern that triggers when sequence of characters given by `chars` is encountered.
    pub fn all_of(chars:&str) -> Self {
        chars.chars().fold(Self::always(),|pat,char| pat >> Self::char(char))
    }

    /// The pattern that triggers on any characters contained in `chars`.
    pub fn any_of(chars:&str) -> Self {
        chars.chars().fold(Self::never(),|pat,char| pat | Self::char(char))
    }

    /// The pattern that doesn't trigger on any character contained in `chars`.
    ///
    /// This pattern will _always_ implicitly include [`Symbol::NULL`] and [`Symbol::EOF_CODE`] in
    /// the excluded characters. If you do not want this behaviour instead use
    /// [`Pattern::none_of_codes`] below.
    pub fn none_of(chars:&str) -> Self {
        let min       = Symbol::NULL.value;
        let max       = Symbol::EOF_CODE.value;
        let iter      = iter::once(min).chain(chars.chars().map(|c| c as u32)).chain(iter::once(max));
        Self::none_of_codes(iter.collect_vec().as_slice())
    }

    /// This pattern doesn't trigger on any code contained in `codes`.
    pub fn none_of_codes(codes:&[u32]) -> Self {
        let mut codes = Vec::from(codes);
        codes.sort();
        codes.dedup();
        let pattern = codes.iter().tuple_windows().fold(Self::never(),|pat,(prev_code,next_code)| {
            let start = prev_code + 1;
            let end   = next_code - 1;
            if end < start { pat } else {
                pat | Pattern::symbols(Symbol::from(start)..=Symbol::from(end))
            }
        });
        if codes.contains(&Symbol::NULL.value) && codes.contains(&Symbol::EOF_CODE.value) {
            pattern
        } else if codes.contains(&Symbol::NULL.value) {
            let last        = codes.last().unwrap() + 1;
            let last_to_eof = Pattern::symbols(Symbol::from(last)..=Symbol::EOF_CODE);
            pattern | last_to_eof
        } else if codes.contains(&Symbol::EOF_CODE.value) {
            let first         = codes.first().unwrap() - 1;
            let null_to_first = Pattern::symbols(Symbol::NULL..=Symbol::from(first));
            null_to_first | pattern
        } else {
            let last          = codes.last().unwrap() + 1;
            let last_to_eof   = Pattern::symbols(Symbol::from(last)..=Symbol::EOF_CODE);
            let first         = codes.first().unwrap() - 1;
            let null_to_first = Pattern::symbols(Symbol::NULL..=Symbol::from(first));
            null_to_first | pattern | last_to_eof
        }
    }

    /// The pattern that triggers on any character but `char`.
    pub fn not(char:char) -> Self {
        Self::none_of(&char.to_string())
    }

    /// The pattern that triggers on any symbol but `symbol`.
    pub fn not_symbol(symbol:Symbol) -> Self {
        if symbol == Symbol::NULL {
            Self::Range(Symbol::from(Symbol::NULL.value + 1)..=Symbol::EOF_CODE)
        } else if symbol == Symbol::EOF_CODE {
            Self::Range(Symbol::NULL..=Symbol::from(Symbol::EOF_CODE.value - 1))
        } else {
            let prev_code = Symbol::from(symbol.value - 1);
            let next_code = Symbol::from(symbol.value + 1);
            let before    = Self::Range(Symbol::NULL..=prev_code);
            let after     = Self::Range(next_code..=Symbol::EOF_CODE);
            before | after
        }
    }

    /// The pattern that triggers on `num` repetitions of `pat`.
    pub fn repeat(pat:&Pattern, num:usize) -> Self {
        (0..num).fold(Self::always(),|p,_| p >> pat.clone())
    }

    /// Pattern that triggers on `min`..`max` repetitions of `pat`.
    pub fn repeat_between(pat:&Pattern, min:usize, max:usize) -> Self {
        (min..max).fold(Self::never(),|p,n| p | Self::repeat(pat,n))
    }
}


// === Trait Impls ====

impl BitOr<Pattern> for Pattern {
    type Output = Pattern;
    fn bitor(self, rhs:Pattern) -> Self::Output {
        match (self, rhs) {
            (Or(mut lhs), Or(    rhs)) => {lhs.extend(rhs)   ; Or(lhs)},
            (Or(mut lhs), rhs        ) => {lhs.push(rhs)     ; Or(lhs)},
            (lhs        , Or(mut rhs)) => {rhs.insert(0,lhs) ; Or(rhs)},
            (lhs        , rhs        ) => Or(vec![lhs,rhs]),
        }
    }
}

impl BitOr<Pattern> for &Pattern {
    type Output = Pattern;

    fn bitor(self, rhs:Pattern) -> Self::Output {
        self.clone() | rhs
    }
}

impl BitOr<&Pattern> for Pattern {
    type Output = Pattern;

    fn bitor(self, rhs:&Pattern) -> Self::Output {
        self | rhs.clone()
    }
}

impl BitOr<&Pattern> for &Pattern {
    type Output = Pattern;

    fn bitor(self, rhs:&Pattern) -> Self::Output {
        self.clone() | rhs.clone()
    }
}

impl Shr<Pattern> for Pattern {
    type Output = Pattern;
    fn shr(self, rhs:Pattern) -> Self::Output {
        match (self, rhs) {
            (Seq(mut lhs), Seq(rhs)    ) => {lhs.extend(rhs)   ; Seq(lhs)},
            (Seq(mut lhs), rhs         ) => {lhs.push(rhs)     ; Seq(lhs)},
            (lhs         , Seq(mut rhs)) => {rhs.insert(0,lhs) ; Seq(rhs)},
            (lhs         , rhs         ) => Seq(vec![lhs, rhs]),
        }
    }
}

impl Shr<Pattern> for &Pattern {
    type Output = Pattern;

    fn shr(self, rhs:Pattern) -> Self::Output {
        self.clone() >> rhs
    }
}

impl Shr<&Pattern> for Pattern {
    type Output = Pattern;

    fn shr(self, rhs:&Pattern) -> Self::Output {
        self >> rhs.clone()
    }
}

impl Shr<&Pattern> for &Pattern {
    type Output = Pattern;

    fn shr(self, rhs:&Pattern) -> Self::Output {
        self.clone() >> rhs.clone()
    }
}



// =================
// === Utilities ===
// =================

/// Quote a character as a character pattern.
///
/// It is equivalent to `Pattern::char(...)`.
#[macro_export]
macro_rules! c {
    ($char:literal) => {
        Pattern::char($char)
    }
}

/// Quote a string as a literal pattern.
///
/// It is equivalent to `Pattern::all_of(...)`.
#[macro_export]
macro_rules! l {
    ($lit:literal) => {
        Pattern::all_of($lit)
    }
}



// =============
// === Tests ===
// =============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_many1() {
        let many1    = l!("abc").many1();
        let expected = l!("abc") >> Pattern::Many(Box::new(l!("abc")));
        assert_eq!(many1,expected);
    }

    #[test]
    fn pattern_opt() {
        let opt      = l!("abc").opt();
        let expected = l!("abc") | Pattern::Always;
        assert_eq!(opt,expected);
    }

    #[test]
    fn pattern_all_of() {
        let all_of   = Pattern::all_of("abcde");
        let expected = Pattern::Seq(vec![Pattern::Always,c!('a'),c!('b'),c!('c'),c!('d'),c!('e')]);
        assert_eq!(all_of,expected);
    }

    #[test]
    fn pattern_any_of() {
        let any_of   = Pattern::any_of("abcde");
        let expected = Pattern::Or(vec![Pattern::Never,c!('a'),c!('b'),c!('c'),c!('d'),c!('e')]);
        assert_eq!(any_of,expected);
    }

    #[test]
    fn pattern_none_of() {
        let none_of  = Pattern::none_of("be");
        let expected = Pattern::Never
                     | Pattern::Range(Symbol::from(Symbol::NULL.value + 1)..=Symbol::from('a'))
                     | Pattern::Range(Symbol::from('c')..=Symbol::from('d'))
                     | Pattern::Range(Symbol::from('f')..=Symbol::from(Symbol::EOF_CODE.value - 1));
        assert_eq!(none_of,expected);
    }

    #[test]
    fn pattern_none_of_codes() {
        let none_of  = Pattern::none_of_codes(&[33,37]);
        let expected = Pattern::Range(Symbol::NULL..=Symbol::from(32))
                     | Pattern::Never
                     | Pattern::Range(Symbol::from(34)..=Symbol::from(36))
                     | Pattern::Range(Symbol::from(38)..=Symbol::EOF_CODE);
        assert_eq!(none_of,expected);
    }

    #[test]
    fn pattern_not() {
        let not      = Pattern::not('a');
        let expected = Pattern::none_of("a");
        assert_eq!(not,expected);
    }

    #[test]
    fn pattern_not_symbol() {
        let symbol = Symbol::from('d');
        let not_symbol = Pattern::not_symbol(symbol);
        let expected = Pattern::Range(Symbol::NULL..=Symbol::from('c'))
                     | Pattern::Range(Symbol::from('e')..=Symbol::EOF_CODE);
        assert_eq!(not_symbol,expected);
    }

    #[test]
    fn pattern_repeat() {
        let repeat   = Pattern::repeat(&c!('a'),5);
        let expected = Pattern::all_of("aaaaa");
        assert_eq!(repeat,expected);
    }

    #[test]
    fn pattern_repeat_between() {
        let repeat_between = Pattern::repeat_between(&c!('a'),2,4);
        let expected       = Pattern::never() | Pattern::all_of("aa") | Pattern::all_of("aaa");
        assert_eq!(repeat_between,expected);
    }

    #[test]
    fn pattern_operator_shr() {
        let pattern_left  = Pattern::char('a');
        let pattern_right = Pattern::not_symbol(Symbol::EOF_CODE);
        let val_val       = pattern_left.clone() >> pattern_right.clone();
        let ref_val       = &pattern_left >> pattern_right.clone();
        let val_ref       = pattern_left.clone() >> &pattern_right;
        let ref_ref       = &pattern_left >> &pattern_right;
        let expected      = Pattern::Seq(vec![pattern_left,pattern_right]);
        assert_eq!(val_val,expected);
        assert_eq!(ref_val,expected);
        assert_eq!(val_ref,expected);
        assert_eq!(ref_ref,expected);
    }

    #[test]
    fn pattern_operator_shr_collapse() {
        let seq = Pattern::Seq(vec![c!('a'),c!('b')]);
        let lit = c!('c');
        assert_eq!(&seq >> &seq,Pattern::Seq(vec![c!('a'),c!('b'),c!('a'),c!('b')]));
        assert_eq!(&seq >> &lit,Pattern::Seq(vec![c!('a'),c!('b'),c!('c')]));
        assert_eq!(&lit >> &seq,Pattern::Seq(vec![c!('c'),c!('a'),c!('b')]));
        assert_eq!(&lit >> &lit,Pattern::Seq(vec![c!('c'),c!('c')]));
    }

    #[test]
    fn pattern_operator_bit_or() {
        let pattern_left  = Pattern::char('a');
        let pattern_right = Pattern::not_symbol(Symbol::EOF_CODE);
        let val_val       = pattern_left.clone() | pattern_right.clone();
        let ref_val       = &pattern_left | pattern_right.clone();
        let val_ref       = pattern_left.clone() | &pattern_right;
        let ref_ref       = &pattern_left | &pattern_right;
        let expected      = Pattern::Or(vec![pattern_left,pattern_right]);
        assert_eq!(val_val,expected);
        assert_eq!(ref_val,expected);
        assert_eq!(val_ref,expected);
        assert_eq!(ref_ref,expected);
    }

    #[test]
    fn pattern_operator_bit_or_collapse() {
        let seq = Pattern::Or(vec![c!('a'),c!('b')]);
        let lit = c!('c');
        assert_eq!(&seq | &seq,Pattern::Or(vec![c!('a'),c!('b'),c!('a'),c!('b')]));
        assert_eq!(&seq | &lit,Pattern::Or(vec![c!('a'),c!('b'),c!('c')]));
        assert_eq!(&lit | &seq,Pattern::Or(vec![c!('c'),c!('a'),c!('b')]));
        assert_eq!(&lit | &lit,Pattern::Or(vec![c!('c'),c!('c')]));
    }

    #[test]
    fn pattern_macro_character() {
        let with_macro = c!('c');
        let explicit   = Pattern::char('c');
        assert_eq!(with_macro,explicit);
    }

    #[test]
    fn pattern_macro_literal() {
        let with_macro = l!("abcde");
        let explicit   = Pattern::all_of("abcde");
        assert_eq!(with_macro,explicit);
    }
}
