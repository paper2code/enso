//! The structure for defining deterministic finite automata.

use crate::automata::alphabet;
use crate::automata::nfa::NFA;
use crate::automata::state;
use crate::automata::data::matrix::Matrix;

use std::collections::BTreeSet;
use std::collections::HashMap;

use crate::prelude::*;



// =============
// === Types ===
// =============

/// A state identifier based on a set of states.
///
/// This is used during the NFA -> DFA transformation, where multiple states can merge together due
/// to the collapsing of epsilon transitions.
pub type StateSetId = BTreeSet<state::Identifier>;



// =====================================
// === Deterministic Finite Automata ===
// =====================================

/// The definition of a [DFA](https://en.wikipedia.org/wiki/Deterministic_finite_automaton) for a
/// given set of symbols, states, and transitions.
///
/// A DFA is a finite state automaton that accepts or rejects a given sequence of symbols by
/// executing on a sequence of states _uniquely_ determined by the sequence of input symbols.
///
/// ```text
///  ┌───┐  'D'  ┌───┐  'F'  ┌───┐  'A'  ┌───┐
///  │ 0 │ ----> │ 1 │ ----> │ 2 │ ----> │ 3 │
///  └───┘       └───┘       └───┘       └───┘
/// ```
#[derive(Clone,Debug,Default,Eq,PartialEq)]
pub struct DFA {
    /// A set of disjoint intervals over the allowable input alphabet.
    pub alphabet_segmentation:alphabet::Segmentation,
    /// The transition matrix for the DFA.
    ///
    /// It represents a function of type `(state, symbol) -> state`, returning the identifier for
    /// the new state.
    ///
    /// For example, the transition matrix for an automaton that accepts the language
    /// `{"A" | "B"}*"` would appear as follows, with `-` denoting
    /// [the invalid state](state::Identifier::INVALID). The leftmost column encodes the input
    /// state, while the topmost row encodes the input symbols.
    ///
    /// |   | A | B |
    /// |:-:|:-:|:-:|
    /// | 0 | 1 | - |
    /// | 1 | - | 0 |
    ///
    pub links:Matrix<state::Identifier>,
    /// A collection of callbacks for each state (indexable in order)
    pub callbacks:Vec<Option<RuleExecutable>>,
}

impl DFA {
    /// Check whether the DFA has a rule for the target state.
    ///
    /// This method should only be used in generated code, where its invariants are already checked.
    pub fn has_rule_for(&self, target_state:state::Identifier) -> bool {
        let callback = self.callbacks.get(target_state.id);
        callback.is_some() && callback.unwrap().is_some()
    }
}


// === Trait Impls ===

impl From<&NFA> for DFA {

    /// Transforms an NFA into a DFA, based on the algorithm described
    /// [here](https://www.youtube.com/watch?v=taClnxU-nao).
    /// The asymptotic complexity is quadratic in number of states.
    fn from(nfa:&NFA) -> Self {
        let     nfa_mat     = nfa.nfa_matrix();
        let     eps_mat     = nfa.eps_matrix();
        let mut dfa_mat     = Matrix::new(0,nfa.alphabet_segmentation.len());
        let mut dfa_eps_ixs = Vec::<StateSetId>::new();
        let mut dfa_eps_map = HashMap::<StateSetId,state::Identifier>::new();

        dfa_eps_ixs.push(eps_mat[0].clone());
        dfa_eps_map.insert(eps_mat[0].clone(),state::Identifier::from(0));

        let mut i = 0;
        while i < dfa_eps_ixs.len()  {
            dfa_mat.new_row();
            for voc_ix in 0..nfa.alphabet_segmentation.len() {
                let mut eps_set = StateSetId::new();
                for &eps_ix in &dfa_eps_ixs[i] {
                    let tgt = nfa_mat[(eps_ix.id,voc_ix)];
                    if tgt != state::Identifier::INVALID {
                        eps_set.extend(eps_mat[tgt.id].iter());
                    }
                }
                if !eps_set.is_empty() {
                    dfa_mat[(i,voc_ix)] = match dfa_eps_map.get(&eps_set) {
                        Some(&id) => id,
                        None => {
                            let id = state::Identifier::new(dfa_eps_ixs.len());
                            dfa_eps_ixs.push(eps_set.clone());
                            dfa_eps_map.insert(eps_set,id);
                            id
                        },
                    };
                }
            }
            i += 1;
        }

        let mut callbacks = vec![None; dfa_eps_ixs.len()];
        let     priority  = dfa_eps_ixs.len();
        for (dfa_ix, epss) in dfa_eps_ixs.into_iter().enumerate() {
            let has_name = |&key:&state::Identifier| nfa.states[key.id].name().is_some();
            if let Some(eps) = epss.into_iter().find(has_name) {
                let code          = nfa.states[eps.id].name().as_ref().cloned().unwrap();
                callbacks[dfa_ix] = Some(RuleExecutable {code,priority});
            }
        }

        let alphabet_segmentation = nfa.alphabet_segmentation.clone();
        let links = dfa_mat;

        DFA{alphabet_segmentation,links,callbacks}
    }
}

impl From<Vec<Vec<usize>>> for Matrix<state::Identifier> {
    fn from(input:Vec<Vec<usize>>) -> Self {
        let rows        = input.len();
        let columns     = if rows == 0 {0} else {input[0].len()};
        let mut matrix  = Self::new(rows,columns);
        for row in 0..rows {
            for column in 0..columns {
                matrix[(row,column)] = state::Identifier::from(input[row][column]);
            }
        }
        matrix
    }
}



// ================
// === Callback ===
// ================

/// The callback associated with an arbitrary state of a finite automaton.
///
/// It contains the rust code that is intended to be executed after encountering a
/// [`pattern`](super::pattern::Pattern) that causes the associated state transition. This pattern
/// is declared in [`Rule.pattern`](crate::group::rule::Rule::pattern).
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RuleExecutable {
    /// A description of the priority with which the callback is constructed during codegen.
    pub priority:usize,
    /// The rust code that will be executed when running this callback.
    pub code:String,
}

impl RuleExecutable {
    /// Creates a new rule executable with the provided `priority` and `code`.
    pub fn new(priority:usize, code_str:impl Into<String>) -> RuleExecutable {
        let code = code_str.into();
        RuleExecutable{priority,code}
    }
}



// =============
// === Tests ===
// =============

#[cfg(test)]
pub mod tests {
    extern crate test;
    use super::*;
    use crate::automata::state;
    use crate::automata::nfa;
    use test::Bencher;

    // === Utilities ===

    const INVALID:usize = state::Identifier::INVALID.id;

    fn assert_same_alphabet(dfa:&DFA, nfa:&NFA) {
        assert_eq!(dfa.alphabet_segmentation,nfa.alphabet_segmentation);
    }

    fn assert_same_matrix(dfa:&DFA, expected:&Matrix<state::Identifier>) {
        assert_eq!(dfa.links,*expected);
    }


    // === The Tests ===

    #[test]
    fn dfa_pattern_range() {
        let nfa = nfa::tests::pattern_range();
        let dfa = DFA::from(&nfa.nfa);
        assert_same_alphabet(&dfa,&nfa);
        let expected = Matrix::from(
            vec![
                vec![INVALID , 1       , INVALID],
                vec![INVALID , INVALID , INVALID],
            ]
        );
        assert_same_matrix(&dfa,&expected);
    }

    #[test]
    fn dfa_pattern_or() {
        let nfa = nfa::tests::pattern_or();
        let dfa = DFA::from(&nfa.nfa);
        assert_same_alphabet(&dfa,&nfa);
        let expected = Matrix::from(
            vec![
                vec![INVALID , 1       , INVALID , 2       , INVALID],
                vec![INVALID , INVALID , INVALID , INVALID , INVALID],
                vec![INVALID , INVALID , INVALID , INVALID , INVALID],
            ]
        );
        assert_same_matrix(&dfa,&expected);
    }

    #[test]
    fn dfa_pattern_seq() {
        let nfa = nfa::tests::pattern_seq();
        let dfa = DFA::from(&nfa.nfa);
        assert_same_alphabet(&dfa,&nfa);
        let expected = Matrix::from(
            vec![
                vec![INVALID , 1       , INVALID , INVALID , INVALID],
                vec![INVALID , INVALID , INVALID , 2       , INVALID],
                vec![INVALID , INVALID , INVALID , INVALID , INVALID],
            ]
        );
        assert_same_matrix(&dfa,&expected);
    }

    #[test]
    fn dfa_pattern_many() {
        let nfa = nfa::tests::pattern_many();
        let dfa = DFA::from(&nfa.nfa);
        assert_same_alphabet(&dfa,&nfa);
        let expected = Matrix::from(
            vec![
                vec![INVALID , 1 , INVALID],
                vec![INVALID , 1 , INVALID],
            ]
        );
        assert_same_matrix(&dfa,&expected);
    }

    #[test]
    fn dfa_pattern_always() {
        let nfa = nfa::tests::pattern_always();
        let dfa = DFA::from(&nfa.nfa);
        assert_same_alphabet(&dfa,&nfa);
        let expected = Matrix::from(
            vec![
                vec![INVALID]
            ]
        );
        assert_same_matrix(&dfa,&expected);
    }

    #[test]
    fn dfa_pattern_never() {
        let nfa = nfa::tests::pattern_never();
        let dfa = DFA::from(&nfa.nfa);
        assert_same_alphabet(&dfa,&nfa);
        let expected = Matrix::from(
            vec![
                vec![INVALID]
            ]
        );
        assert_same_matrix(&dfa,&expected);
    }

    #[test]
    fn dfa_simple_rules() {
        let nfa = nfa::tests::simple_rules();
        let dfa = DFA::from(&nfa.nfa);
        assert_same_alphabet(&dfa,&nfa);
        let expected = Matrix::from(
            vec![
                vec![INVALID , 1       , INVALID , INVALID],
                vec![INVALID , INVALID , 2       , INVALID],
                vec![INVALID , INVALID , INVALID , INVALID],
            ]
        );
        assert_same_matrix(&dfa,&expected);
    }

    #[test]
    fn dfa_complex_rules() {
        let nfa = nfa::tests::complex_rules();
        let dfa = DFA::from(&nfa.nfa);
        assert_same_alphabet(&dfa,&nfa);
        let expected = Matrix::from(
            vec![
                vec![1       , 2       , 1       , 1       , 1       , 1       , 3]      ,
                vec![INVALID , INVALID , INVALID , INVALID , INVALID , INVALID , INVALID],
                vec![INVALID , INVALID , INVALID , 4       , 5       , INVALID , INVALID],
                vec![INVALID , INVALID , INVALID , INVALID , INVALID , INVALID , INVALID],
                vec![INVALID , INVALID , INVALID , 6       , INVALID , INVALID , INVALID],
                vec![INVALID , INVALID , INVALID , INVALID , 7       , INVALID , INVALID],
                vec![INVALID , INVALID , INVALID , 6       , INVALID , INVALID , INVALID],
                vec![INVALID , INVALID , INVALID , INVALID , 7       , INVALID , INVALID],
            ]
        );
        assert_same_matrix(&dfa,&expected);
    }


    // === The Benchmarks ===

    #[bench]
    fn bench_to_dfa_pattern_range(bencher:&mut Bencher) {
        bencher.iter(|| DFA::from(&nfa::tests::pattern_range().nfa))
    }

    #[bench]
    fn bench_to_dfa_pattern_or(bencher:&mut Bencher) {
        bencher.iter(|| DFA::from(&nfa::tests::pattern_or().nfa))
    }

    #[bench]
    fn bench_to_dfa_pattern_seq(bencher:&mut Bencher) {
        bencher.iter(|| DFA::from(&nfa::tests::pattern_seq().nfa))
    }

    #[bench]
    fn bench_to_dfa_pattern_many(bencher:&mut Bencher) {
        bencher.iter(|| DFA::from(&nfa::tests::pattern_many().nfa))
    }

    #[bench]
    fn bench_to_dfa_pattern_always(bencher:&mut Bencher) {
        bencher.iter(|| DFA::from(&nfa::tests::pattern_always().nfa))
    }

    #[bench]
    fn bench_to_dfa_pattern_never(bencher:&mut Bencher) {
        bencher.iter(|| DFA::from(&nfa::tests::pattern_never().nfa))
    }

    #[bench]
    fn bench_to_dfa_simple_rules(bencher:&mut Bencher) {
        bencher.iter(|| DFA::from(&nfa::tests::simple_rules().nfa))
    }

    #[bench]
    fn bench_to_dfa_complex_rules(bencher:&mut Bencher) {
        bencher.iter(|| DFA::from(&nfa::tests::complex_rules().nfa))
    }
}
